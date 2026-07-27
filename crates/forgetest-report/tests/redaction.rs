use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use forgetest_core::agent::{AgentEvent, AgentEventKind, AgentIdentity, AgentUsage};
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
