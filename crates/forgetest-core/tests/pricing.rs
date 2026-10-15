use forgetest_core::agent::AgentUsage;
use forgetest_core::pricing::PricingTable;
use forgetest_core::repository_report::{ExecutionPolicyManifest, ExecutionPolicyParameters};

const VALID: &str = r#"
schema_version = 1
retrieved_on = "2026-10-15"

[[models]]
model = "example-model-2026-01-01"
input_usd_per_mtok = 2.0
cache_read_usd_per_mtok = 0.5
cache_write_usd_per_mtok = 4.0
output_usd_per_mtok = 10.0
"#;

#[test]
fn pricing_table_prices_normalized_usage_for_exact_models_only() {
    let table = PricingTable::parse(VALID).unwrap();
    let usage = AgentUsage {
        input_tokens: 1_000,
        cached_tokens: 500,
        cache_write_tokens: 100,
        output_tokens: 200,
        reasoning_tokens: 150,
        ..AgentUsage::default()
    };

    let cost = table.cost("example-model-2026-01-01", &usage).unwrap();

    // Reasoning is part of output and is never priced separately.
    assert!((cost - 0.00465).abs() < 1e-12);
    for near_miss in [
        "EXAMPLE-MODEL-2026-01-01",
        "example-model",
        "example-model-2026-01-01-latest",
        " example-model-2026-01-01",
    ] {
        assert!(table.cost(near_miss, &usage).is_none(), "{near_miss}");
    }
}

#[test]
fn pricing_table_rejects_ambiguous_or_impossible_prices() {
    let cases = [
        (VALID.replace("schema_version = 1", "schema_version = 2"), "schema version"),
        (
            VALID.replace("retrieved_on = \"2026-10-15\"", "retrieved_on = \"15/10/2026\""),
            "retrieved_on",
        ),
        (VALID.replace("output_usd_per_mtok = 10.0", "output_usd_per_mtok = -1.0"), "non-negative"),
        (VALID.replace("output_usd_per_mtok = 10.0", "output_usd_per_mtok = nan"), "finite"),
        (VALID.replace("output_usd_per_mtok = 10.0", "output_usd_per_mtok = inf"), "finite"),
        (VALID.replace("output_usd_per_mtok = 10.0\n", ""), "output_usd_per_mtok"),
        (format!("{VALID}currency = \"EUR\"\n"), "unknown field"),
        (
            VALID.replace("output_usd_per_mtok = 10.0", "output_usd_per_mtok = 10.0\nnote = \"x\""),
            "unknown field",
        ),
        (
            format!(
                "{VALID}\n[[models]]\nmodel = \"example-model-2026-01-01\"\ninput_usd_per_mtok = 1.0\ncache_read_usd_per_mtok = 0.1\ncache_write_usd_per_mtok = 1.0\noutput_usd_per_mtok = 5.0\n"
            ),
            "duplicate",
        ),
        (
            VALID.replace("model = \"example-model-2026-01-01\"", "model = \"example model\""),
            "whitespace",
        ),
        (
            "schema_version = 1\nretrieved_on = \"2026-10-15\"\nmodels = []\n".to_string(),
            "no models",
        ),
    ];

    for (content, expected) in cases {
        let error = format!("{:#}", PricingTable::parse(&content).unwrap_err());
        assert!(
            error.contains(expected),
            "expected '{expected}' in: {error}"
        );
    }
}

#[test]
fn pricing_file_errors_do_not_echo_the_host_path() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("pricing.toml");
    std::fs::write(&invalid, VALID.replace("10.0", "-10.0")).unwrap();
    let missing = directory.path().join("missing-pricing.toml");

    for path in [invalid, missing] {
        let error = format!("{:#}", PricingTable::load(&path).unwrap_err());
        assert!(error.contains("pricing table"), "{error}");
        assert!(
            !error.contains(&directory.path().display().to_string()),
            "{error}"
        );
    }
}

#[test]
fn pricing_table_order_does_not_change_the_policy_digest() {
    let second = "\n[[models]]\nmodel = \"another-model-2026-01-01\"\ninput_usd_per_mtok = 1.0\ncache_read_usd_per_mtok = 0.1\ncache_write_usd_per_mtok = 1.0\noutput_usd_per_mtok = 5.0\n";
    let (header, first) = VALID.split_once("\n[[models]]").unwrap();
    let forward = PricingTable::parse(&format!("{VALID}{second}")).unwrap();
    let reversed = PricingTable::parse(&format!("{header}{second}\n[[models]]{first}")).unwrap();

    assert_eq!(forward, reversed);
    assert_eq!(forward.models[0].model, "another-model-2026-01-01");
    let digest = |table: &PricingTable| {
        ExecutionPolicyManifest {
            schema_version: 1,
            profile: "benchmark".into(),
            agent_environment: "docker".into(),
            verifier_environment: "docker".into(),
            verifier_image: None,
            network: "none".into(),
            parameters: ExecutionPolicyParameters {
                pricing: Some(table.clone()),
                ..Default::default()
            },
            digest: String::new(),
        }
        .sealed()
        .digest
    };
    assert_eq!(digest(&forward), digest(&reversed));
}

#[test]
fn pricing_file_with_a_future_retrieval_date_is_rejected_when_read() {
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(2))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let future = VALID.replace("2026-10-15", &tomorrow);

    let error = format!("{:#}", PricingTable::parse(&future).unwrap_err());
    assert!(error.contains("later than today"), "{error}");

    // Stored tables are validated without the clock so reports stay deterministic.
    let mut stored = PricingTable::parse(VALID).unwrap();
    stored.retrieved_on = tomorrow;
    stored.validate().unwrap();
}
