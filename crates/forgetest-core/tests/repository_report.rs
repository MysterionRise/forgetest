use std::collections::BTreeMap;

use chrono::Utc;
use forgetest_core::agent::{AgentEvent, AgentEventKind, AgentIdentity, AgentUsage, CostSource};
use forgetest_core::pricing::{ModelPricing, PricingTable};
use forgetest_core::repository_report::{
    Compatibility, ContainerLimitsManifest, ExecutionPolicyManifest, ExecutionPolicyParameters,
    FileChange, FileChangeKind, RepositoryReport, RepositorySuiteSummary, TrialResult, TrialStatus,
};
use uuid::Uuid;

fn trial(task: &str, index: u32, status: TrialStatus) -> TrialResult {
    trial_for_agent(
        task,
        index,
        status,
        "scripted",
        "deterministic",
        "agent-policy",
    )
}

fn trial_for_agent(
    task: &str,
    index: u32,
    status: TrialStatus,
    adapter: &str,
    model: &str,
    configuration_digest: &str,
) -> TrialResult {
    TrialResult {
        id: Uuid::new_v4(),
        task_id: task.into(),
        task_digest: format!("digest-{task}"),
        agent: AgentIdentity {
            adapter: adapter.into(),
            adapter_version: "1".into(),
            executable_sha256: None,
            model: model.into(),
            configuration_digest: configuration_digest.into(),
        },
        environment_digest: "env".into(),
        trial_index: index,
        agent_attempts: 1,
        status,
        changed_files: vec![FileChange {
            path: "src/lib.rs".into(),
            kind: FileChangeKind::Modified,
            before_sha256: Some("before".into()),
            after_sha256: Some("after".into()),
        }],
        patch: "diff --git a/src/lib.rs b/src/lib.rs".into(),
        grader: None,
        events: vec![AgentEvent {
            sequence: 1,
            timestamp: Utc::now(),
            kind: AgentEventKind::Message,
            message: "edited src/lib.rs".into(),
            raw: None,
        }],
        usage: AgentUsage::default(),
        duration_ms: 10,
        termination_reason: None,
        artifacts: BTreeMap::new(),
        error: None,
    }
}

fn report(trials: Vec<TrialResult>) -> RepositoryReport {
    RepositoryReport::new(
        RepositorySuiteSummary {
            id: "suite".into(),
            name: "Suite".into(),
            digest: "suite-digest".into(),
            task_digests: BTreeMap::from([
                ("a".into(), "digest-a".into()),
                ("b".into(), "digest-b".into()),
            ]),
        },
        ExecutionPolicyManifest {
            schema_version: 1,
            profile: "benchmark".into(),
            agent_environment: "container".into(),
            verifier_environment: "docker".into(),
            verifier_image: Some("runner@sha256:abc".into()),
            network: "none".into(),
            digest: "policy".into(),
            parameters: ExecutionPolicyParameters::default(),
        },
        trials,
        100,
    )
}

#[test]
fn aggregate_includes_every_scheduled_trial() {
    let report = report(vec![
        trial("a", 1, TrialStatus::Passed),
        trial("a", 2, TrialStatus::Passed),
        trial("a", 3, TrialStatus::Failed),
        trial("b", 1, TrialStatus::AgentError),
        trial("b", 2, TrialStatus::Timeout),
        trial("b", 3, TrialStatus::Passed),
    ]);

    let stats = report
        .aggregate
        .per_agent
        .get("scripted/deterministic@1#agent-policy")
        .unwrap();
    assert_eq!(stats.scheduled, 6);
    assert_eq!(stats.passed, 3);
    assert_eq!(stats.task_failures, 1);
    assert_eq!(stats.agent_errors, 1);
    assert_eq!(stats.timeouts, 1);
    assert_eq!(stats.infrastructure_errors, 0);
    assert_eq!(stats.observed_resolution_rate, 0.5);
    assert_eq!(stats.pass_at_1, 0.5);
    assert_eq!(stats.pass_power_3, 0.0);
    assert!(stats.wilson_95_low < 0.5);
    assert!(stats.wilson_95_high > 0.5);
}

#[test]
fn compatibility_requires_matching_suite_tasks_and_policy() {
    let baseline = report(vec![trial("a", 1, TrialStatus::Passed)]);
    let mut current = baseline.clone();
    assert_eq!(
        current.compatibility_with(&baseline),
        Compatibility::Comparable
    );

    current.policy.digest = "different".into();
    assert!(matches!(
        current.compatibility_with(&baseline),
        Compatibility::Incomparable { .. }
    ));
}

#[test]
fn report_v2_json_roundtrip_preserves_statuses() {
    let report = report(vec![trial("a", 1, TrialStatus::GraderError)]);
    let json = serde_json::to_string_pretty(&report).unwrap();
    let loaded: RepositoryReport = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.trials[0].status, TrialStatus::GraderError);
    assert_eq!(
        loaded.aggregate.per_agent["scripted/deterministic@1#agent-policy"].scheduled,
        1
    );
}

#[test]
fn report_v2_save_rejects_invalid_derived_state() {
    let report = report(vec![trial("a", 1, TrialStatus::Passed)]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");

    let error = report.save_json(&path).unwrap_err();

    assert!(error.to_string().contains("policy digest"));
    assert!(!path.exists());
}

#[test]
fn report_v2_loader_rejects_tampered_policy() {
    let mut report = report(vec![trial("a", 1, TrialStatus::Passed)]);
    report.policy = report.policy.sealed();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    report.save_json(&path).unwrap();

    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    value["policy"]["parameters"]["parallelism"] = serde_json::json!(999);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let error = RepositoryReport::load_json(&path).unwrap_err();
    assert!(error.to_string().contains("policy digest"));
}

#[test]
fn report_v2_loader_rejects_stale_aggregate() {
    let mut report = report(vec![trial("a", 1, TrialStatus::Passed)]);
    report.policy = report.policy.sealed();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    report.save_json(&path).unwrap();

    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    value["aggregate"]["per_agent"]["scripted/deterministic@1#agent-policy"]["passed"] =
        serde_json::json!(0);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let error = RepositoryReport::load_json(&path).unwrap_err();
    assert!(error.to_string().contains("aggregate"));
}

#[test]
fn comparison_detects_agent_resolution_regression() {
    let baseline = report(vec![
        trial("a", 1, TrialStatus::Passed),
        trial("b", 1, TrialStatus::Passed),
    ]);
    let current = report(vec![
        trial("a", 1, TrialStatus::Passed),
        trial("b", 1, TrialStatus::Failed),
    ]);

    let comparison = current.compare(&baseline, 0.05);

    assert_eq!(comparison.regressions.len(), 1);
    assert_eq!(
        comparison.regressions[0].agent,
        "scripted/deterministic@1#agent-policy"
    );
    assert_eq!(comparison.regressions[0].delta, -0.5);
    assert!(comparison.gating_eligible);
}

#[test]
fn aggregate_does_not_merge_distinct_agent_configurations() {
    let report = report(vec![
        trial_for_agent("a", 1, TrialStatus::Passed, "codex", "model", "config-a"),
        trial_for_agent("a", 1, TrialStatus::Failed, "codex", "model", "config-b"),
    ]);

    assert_eq!(report.aggregate.per_agent.len(), 2);
    assert_eq!(report.agents.len(), 2);
    assert_eq!(
        report.aggregate.per_agent["codex/model@1#config-a"].passed,
        1
    );
    assert_eq!(
        report.aggregate.per_agent["codex/model@1#config-b"].passed,
        0
    );
}

#[test]
fn pairwise_bootstrap_is_task_paired_and_deterministic() {
    let mut trials = Vec::new();
    for task in ["a", "b"] {
        for index in 1..=3 {
            trials.push(trial_for_agent(
                task,
                index,
                TrialStatus::Failed,
                "agent-a",
                "model-a",
                "config-a",
            ));
            trials.push(trial_for_agent(
                task,
                index,
                TrialStatus::Passed,
                "agent-b",
                "model-b",
                "config-b",
            ));
        }
    }

    let first = report(trials.clone());
    let second = report(trials);
    let comparison = &first.aggregate.pairwise[0];

    assert_eq!(comparison.common_tasks, 2);
    assert_eq!(comparison.bootstrap_iterations, 10_000);
    assert_eq!(comparison.delta_b_minus_a, 1.0);
    assert_eq!(comparison.ci_95_low, 1.0);
    assert_eq!(comparison.ci_95_high, 1.0);
    assert_eq!(first.aggregate.pairwise, second.aggregate.pairwise);
}

fn benchmark_policy() -> ExecutionPolicyManifest {
    ExecutionPolicyManifest {
        schema_version: 1,
        profile: "benchmark".into(),
        agent_environment: "docker".into(),
        verifier_environment: "docker".into(),
        verifier_image: Some("runner@sha256:abc".into()),
        network: "agent=bridge;verifier=none".into(),
        parameters: ExecutionPolicyParameters {
            trials: 3,
            parallelism: 2,
            agent_images: BTreeMap::from([("codex".into(), "agent@sha256:def".into())]),
            agent_timeout_secs: 900,
            max_agent_output_bytes: 1024,
            max_agent_tokens: Some(100_000),
            max_agent_cost_usd: Some(10.0),
            agent_retries: 0,
            max_workspace_files: 10_000,
            max_workspace_bytes: 64 * 1024 * 1024,
            max_patch_bytes: 4 * 1024 * 1024,
            verifier_max_output_bytes: 4 * 1024 * 1024,
            agent_container: Some(ContainerLimitsManifest {
                memory: "2g".into(),
                cpus: 2.0,
                pids_limit: 256,
                tmpfs_size: "256m".into(),
            }),
            verifier_container: Some(ContainerLimitsManifest {
                memory: "1g".into(),
                cpus: 1.0,
                pids_limit: 128,
                tmpfs_size: "256m".into(),
            }),
            max_run_cost_usd: None,
            pricing: None,
        },
        digest: String::new(),
    }
    .sealed()
}

fn pricing_table() -> PricingTable {
    PricingTable {
        schema_version: 1,
        retrieved_on: "2026-10-15".into(),
        models: vec![ModelPricing {
            model: "example-model-2026-01-01".into(),
            input_usd_per_mtok: 1.0,
            cache_read_usd_per_mtok: 0.1,
            cache_write_usd_per_mtok: 1.25,
            output_usd_per_mtok: 8.0,
        }],
    }
}

#[test]
fn execution_policy_digest_covers_limits_and_images() {
    let policy = benchmark_policy();

    assert!(policy.verify_digest());
    assert_eq!(policy.digest.len(), 64);

    let mut changed = policy.clone();
    changed.parameters.max_agent_tokens = Some(100_001);
    assert!(!changed.verify_digest());
    assert_ne!(changed.sealed().digest, policy.digest);
}

/// Digest of `benchmark_policy()` computed before pricing and the run cost
/// cap existed (baseline c3b8f9a). Benchmark locks record this value.
const PRE_PRICING_POLICY_DIGEST: &str =
    "a4041e082953c80e170e02c35c567e1fd1fd57fbcddcfa33a1d1dfa4ebe8d771";

#[test]
fn execution_policy_digest_is_unchanged_when_new_options_are_unused() {
    let policy = benchmark_policy();
    let parameters = serde_json::to_value(&policy.parameters).unwrap();

    assert_eq!(policy.digest, PRE_PRICING_POLICY_DIGEST);
    assert!(parameters.get("max_run_cost_usd").is_none());
    assert!(parameters.get("pricing").is_none());

    let mut capped = policy.clone();
    capped.parameters.max_run_cost_usd = Some(5.0);
    let capped = capped.sealed();
    let mut priced = policy.clone();
    priced.parameters.pricing = Some(pricing_table());
    let priced = priced.sealed();
    assert_ne!(capped.digest, PRE_PRICING_POLICY_DIGEST);
    assert_ne!(priced.digest, PRE_PRICING_POLICY_DIGEST);
    assert_ne!(capped.digest, priced.digest);

    let mut repriced = priced.clone();
    repriced.parameters.pricing.as_mut().unwrap().models[0].output_usd_per_mtok = 9.0;
    assert!(!repriced.verify_digest());
}

#[test]
fn committed_v2_reports_load_and_reserialize_byte_for_byte() {
    for path in [
        "../../examples/reports/repository-local/report.json",
        "../../examples/reports/repository-docker/report.json",
    ] {
        let original = std::fs::read_to_string(path).unwrap();
        let report = RepositoryReport::load_json(std::path::Path::new(path)).unwrap();

        assert!(report.policy.verify_digest());
        assert!(report.policy.parameters.pricing.is_none());
        assert!(report
            .aggregate
            .per_agent
            .values()
            .all(|stats| stats.cost_unavailable_trials.is_none()));
        assert_eq!(serde_json::to_string_pretty(&report).unwrap(), original);
    }
}

fn costed_trial(task: &str, index: u32, cost: f64, source: Option<CostSource>) -> TrialResult {
    let mut trial = trial(task, index, TrialStatus::Passed);
    trial.usage = AgentUsage {
        input_tokens: 100,
        output_tokens: 10,
        estimated_cost_usd: cost,
        cost_source: source,
        ..AgentUsage::default()
    };
    trial
}

#[test]
fn aggregate_excludes_unavailable_cost_and_counts_it() {
    let report = report(vec![
        costed_trial("a", 1, 1.0, Some(CostSource::Vendor)),
        costed_trial("a", 2, 0.5, Some(CostSource::PricingTable)),
        costed_trial("b", 1, 9.0, Some(CostSource::Unavailable)),
        costed_trial("b", 2, 0.25, None),
    ]);

    let stats = &report.aggregate.per_agent["scripted/deterministic@1#agent-policy"];
    assert_eq!(stats.total_cost_usd, 1.75);
    assert_eq!(stats.cost_unavailable_trials, Some(1));
    let json = serde_json::to_value(&report.trials[2].usage).unwrap();
    assert_eq!(json["cost_source"], "unavailable");
    assert_eq!(
        serde_json::to_value(&report.trials[1].usage).unwrap()["cost_source"],
        "pricing-table"
    );

    let legacy = report_without_cost_sources();
    let stats = &legacy.aggregate.per_agent["scripted/deterministic@1#agent-policy"];
    assert_eq!(stats.total_cost_usd, 0.75);
    assert_eq!(stats.cost_unavailable_trials, None);
    let json = serde_json::to_value(stats).unwrap();
    assert!(json.get("cost_unavailable_trials").is_none());
    let usage = serde_json::to_value(&legacy.trials[0].usage).unwrap();
    assert!(usage.get("cost_source").is_none());
    assert!(usage.get("cache_write_tokens").is_none());
    assert!(usage.get("reasoning_tokens").is_none());
}

fn report_without_cost_sources() -> RepositoryReport {
    report(vec![
        costed_trial("a", 1, 0.5, None),
        costed_trial("a", 2, 0.25, None),
    ])
}

#[test]
fn report_v2_loader_recomputes_unavailable_cost_count() {
    let mut report = report(vec![
        costed_trial("a", 1, 1.0, Some(CostSource::Vendor)),
        costed_trial("a", 2, 0.0, Some(CostSource::Unavailable)),
    ]);
    report.policy = report.policy.sealed();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    report.save_json(&path).unwrap();
    assert!(RepositoryReport::load_json(&path).is_ok());

    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    value["aggregate"]["per_agent"]["scripted/deterministic@1#agent-policy"]
        ["cost_unavailable_trials"] = serde_json::json!(0);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let error = RepositoryReport::load_json(&path).unwrap_err();
    assert!(error.to_string().contains("aggregate"));
}

#[test]
fn report_with_inline_pricing_round_trips_and_rejects_invalid_prices() {
    let mut report = report(vec![priced_trial()]);
    report.policy.parameters.pricing = Some(pricing_table());
    report.policy.parameters.max_run_cost_usd = Some(2.5);
    report.policy = report.policy.sealed();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    report.save_json(&path).unwrap();

    let loaded = RepositoryReport::load_json(&path).unwrap();
    assert!(loaded.policy.verify_digest());
    assert_eq!(loaded.policy.parameters.pricing, Some(pricing_table()));
    assert_eq!(loaded.policy.parameters.max_run_cost_usd, Some(2.5));

    let mut invalid = report.clone();
    invalid.policy.parameters.pricing.as_mut().unwrap().models[0].input_usd_per_mtok = -1.0;
    invalid.policy = invalid.policy.sealed();
    let error = invalid
        .save_json(&directory.path().join("invalid.json"))
        .unwrap_err();
    assert!(format!("{error:#}").contains("pricing"));
}

fn priced_trial() -> TrialResult {
    let mut trial = trial_for_agent(
        "a",
        1,
        TrialStatus::Passed,
        "codex",
        "example-model-2026-01-01",
        "agent-policy",
    );
    trial.usage = AgentUsage {
        input_tokens: 100,
        output_tokens: 10,
        estimated_cost_usd: 0.001,
        cost_source: Some(CostSource::PricingTable),
        ..AgentUsage::default()
    };
    trial
}

#[test]
fn report_rejects_pricing_table_cost_without_a_matching_policy_entry() {
    let mut unpriced = report(vec![priced_trial()]);
    unpriced.policy = unpriced.policy.sealed();
    let error = unpriced.validate_integrity().unwrap_err();
    assert!(error.to_string().contains("pricing table"), "{error:#}");

    let mut other_model = report(vec![priced_trial()]);
    let mut table = pricing_table();
    table.models[0].model = "example-model-2026-02-01".into();
    other_model.policy.parameters.pricing = Some(table);
    other_model.policy = other_model.policy.sealed();
    assert!(other_model.validate_integrity().is_err());

    let mut priced = report(vec![priced_trial()]);
    priced.policy.parameters.pricing = Some(pricing_table());
    priced.policy = priced.policy.sealed();
    priced.validate_integrity().unwrap();
}

#[test]
fn report_loader_rejects_non_positive_or_non_finite_run_cap() {
    for cap in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut report = report(vec![trial("a", 1, TrialStatus::Passed)]);
        report.policy.parameters.max_run_cost_usd = Some(cap);
        report.policy = report.policy.sealed();
        assert!(report.validate_integrity().is_err(), "cap {cap} validated");

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        assert!(
            RepositoryReport::load_json(&path).is_err(),
            "cap {cap} loaded"
        );
    }
}
