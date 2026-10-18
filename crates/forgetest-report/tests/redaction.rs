use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use forgetest_core::agent::{AgentEvent, AgentEventKind, AgentIdentity, AgentUsage, CostSource};
use forgetest_core::pricing::{ModelPricing, PricingTable};
use forgetest_core::repository_report::{
    ExecutionPolicyManifest, RepositoryReport, RepositorySuiteSummary, TrialResult, TrialStatus,
};
use forgetest_report::redaction::{redact_repository_report, RedactionOptions};
use uuid::Uuid;

#[test]
fn redaction_removes_paths_secrets_and_private_reasoning() {
    let secret = "sk-test-abcdefghijklmnopqrstuvwxyz";
    let report = RepositoryReport::new(
        RepositorySuiteSummary {
            id: "suite".into(),
            name: "Suite".into(),
            digest: "suite".into(),
            task_digests: BTreeMap::from([("task".into(), "task".into())]),
        },
        ExecutionPolicyManifest {
            schema_version: 1,
            profile: "benchmark".into(),
            agent_environment: "container".into(),
            verifier_environment: "docker".into(),
            verifier_image: Some("image@sha256:test".into()),
            network: "none".into(),
            parameters: Default::default(),
            digest: "policy".into(),
        },
        vec![TrialResult {
            id: Uuid::new_v4(),
            task_id: "task".into(),
            task_digest: "task".into(),
            agent: AgentIdentity {
                adapter: "codex".into(),
                adapter_version: "1".into(),
                executable_sha256: None,
                model: "model".into(),
                configuration_digest: "config".into(),
            },
            environment_digest: "env".into(),
            trial_index: 1,
            agent_attempts: 1,
            status: TrialStatus::Failed,
            changed_files: Vec::new(),
            patch: format!("path=/Users/example/private/repo/src/lib.rs\napi_key={secret}\n"),
            grader: None,
            events: vec![
                AgentEvent {
                    sequence: 1,
                    timestamp: Utc::now(),
                    kind: AgentEventKind::Message,
                    message: format!("Bearer {secret}"),
                    raw: Some(serde_json::json!({
                        "type": "message",
                        "reasoning": "private chain of thought",
                        "text": format!("using {secret}")
                    })),
                },
                AgentEvent {
                    sequence: 2,
                    timestamp: Utc::now(),
                    kind: AgentEventKind::Completed,
                    message: "PRIVATE_FINAL_OUTPUT".into(),
                    raw: Some(serde_json::json!({
                        "type": "completed",
                        "result": "PRIVATE_FINAL_OUTPUT"
                    })),
                },
            ],
            usage: AgentUsage::default(),
            duration_ms: 1,
            termination_reason: None,
            artifacts: BTreeMap::from([
                ("trace".into(), "trials/private/trace.jsonl".into()),
                (
                    "grader_stdout".into(),
                    "trials/private/grader.stdout.log".into(),
                ),
            ]),
            error: Some(format!(
                "failed in /Users/example/private/repo with {secret}"
            )),
        }],
        1,
    );

    let redacted = redact_repository_report(
        &report,
        &RedactionOptions {
            path_replacements: vec![(
                PathBuf::from("/Users/example/private/repo"),
                "$WORKSPACE".into(),
            )],
            secret_values: vec![secret.into()],
        },
    )
    .unwrap();
    let json = serde_json::to_string(&redacted).unwrap();

    assert!(redacted.redaction.redacted);
    assert!(redacted.redaction.replacements >= 4);
    assert!(json.contains("$WORKSPACE"));
    assert!(!json.contains("/Users/example/private/repo"));
    assert!(!json.contains(secret));
    assert!(!json.contains("private chain of thought"));
    assert!(!json.contains("\"reasoning\""));
    assert!(redacted.trials[0].artifacts.is_empty());
    assert_eq!(redacted.trials[0].events.len(), 1);
    assert_eq!(redacted.trials[0].events[0].kind, AgentEventKind::Completed);
    assert_eq!(redacted.trials[0].events[0].message, "completed");
    assert!(redacted.trials[0].events[0].raw.is_none());
    assert!(!json.contains("PRIVATE_FINAL_OUTPUT"));
    assert!(!json.contains("trials/private"));
}

#[test]
fn redacted_priced_report_keeps_a_verifiable_inline_pricing_table() {
    let parameters = forgetest_core::repository_report::ExecutionPolicyParameters {
        max_run_cost_usd: Some(10.0),
        pricing: Some(PricingTable {
            schema_version: 1,
            retrieved_on: "2026-10-15".into(),
            models: vec![ModelPricing {
                model: "example-model-2026-01-01".into(),
                input_usd_per_mtok: 1.0,
                cache_read_usd_per_mtok: 0.1,
                cache_write_usd_per_mtok: 0.0,
                output_usd_per_mtok: 8.0,
            }],
        }),
        ..Default::default()
    };
    let private_root = std::env::temp_dir().join("forgetest-private-checkout");
    let report = RepositoryReport::new(
        RepositorySuiteSummary {
            id: "suite".into(),
            name: "Suite".into(),
            digest: "suite".into(),
            task_digests: BTreeMap::from([("task".into(), "task".into())]),
        },
        ExecutionPolicyManifest {
            schema_version: 1,
            profile: "benchmark".into(),
            agent_environment: "docker".into(),
            verifier_environment: "docker".into(),
            verifier_image: Some("image@sha256:test".into()),
            network: "agent=bridge;verifier=none".into(),
            parameters,
            digest: String::new(),
        }
        .sealed(),
        vec![TrialResult {
            id: Uuid::new_v4(),
            task_id: "task".into(),
            task_digest: "task".into(),
            agent: AgentIdentity {
                adapter: "codex".into(),
                adapter_version: "1".into(),
                executable_sha256: None,
                model: "example-model-2026-01-01".into(),
                configuration_digest: "config".into(),
            },
            environment_digest: "env".into(),
            trial_index: 1,
            agent_attempts: 1,
            status: TrialStatus::Passed,
            changed_files: Vec::new(),
            patch: format!("path={}/src/lib.rs\n", private_root.display()),
            grader: None,
            events: Vec::new(),
            usage: AgentUsage {
                input_tokens: 600,
                cached_tokens: 400,
                output_tokens: 100,
                reasoning_tokens: 30,
                estimated_cost_usd: 0.00144,
                cost_source: Some(CostSource::PricingTable),
                ..AgentUsage::default()
            },
            duration_ms: 1,
            termination_reason: None,
            artifacts: BTreeMap::new(),
            error: None,
        }],
        1,
    );

    let redacted = redact_repository_report(
        &report,
        &RedactionOptions {
            path_replacements: vec![(private_root.clone(), "$WORKSPACE".into())],
            secret_values: Vec::new(),
        },
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    redacted.save_json(&path).unwrap();
    let loaded = RepositoryReport::load_json(&path).unwrap();

    assert!(loaded.redaction.redacted);
    assert!(loaded.policy.verify_digest());
    assert_eq!(loaded.policy.digest, report.policy.digest);
    assert_eq!(
        loaded.policy.parameters.pricing,
        report.policy.parameters.pricing
    );
    assert_eq!(loaded.trials[0].usage.reasoning_tokens, 30);
    assert_eq!(
        loaded.trials[0].usage.cost_source,
        Some(CostSource::PricingTable)
    );
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains(&private_root.display().to_string()));
}

#[test]
fn redacting_a_committed_report_preserves_its_digest_and_aggregates() {
    let report = RepositoryReport::load_json(std::path::Path::new(
        "../../examples/reports/repository-local/report.json",
    ))
    .unwrap();

    let redacted = redact_repository_report(&report, &RedactionOptions::default()).unwrap();

    redacted.validate_integrity().unwrap();
    assert_eq!(redacted.policy.digest, report.policy.digest);
    assert_eq!(
        serde_json::to_value(&redacted.aggregate).unwrap(),
        serde_json::to_value(&report.aggregate).unwrap()
    );
}
