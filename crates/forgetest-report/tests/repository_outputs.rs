use std::collections::BTreeMap;

use forgetest_core::agent::{AgentEvent, AgentEventKind, AgentIdentity, AgentUsage};
use forgetest_core::repository_report::{
    ExecutionPolicyManifest, GraderCheck, GraderCheckKind, GraderOutcome, PairwiseAgentComparison,
    RepositoryReport, RepositorySuiteSummary, TrialResult, TrialStatus,
};
use forgetest_report::html::generate_repository_html;
use forgetest_report::sarif::generate_repository_sarif;
use uuid::Uuid;

fn report() -> RepositoryReport {
    RepositoryReport::new(
        RepositorySuiteSummary {
            id: "suite".into(),
            name: "Agent Suite".into(),
            digest: "suite-digest".into(),
            task_digests: BTreeMap::from([("task".into(), "task-digest".into())]),
        },
        ExecutionPolicyManifest {
            schema_version: 1,
            profile: "benchmark".into(),
            agent_environment: "container".into(),
            verifier_environment: "docker".into(),
            verifier_image: Some("runner@sha256:abc".into()),
            network: "none".into(),
            parameters: Default::default(),
            digest: "policy".into(),
        },
        vec![TrialResult {
            id: Uuid::new_v4(),
            task_id: "task".into(),
            task_digest: "task-digest".into(),
            agent: AgentIdentity {
                adapter: "codex".into(),
                adapter_version: "1".into(),
                executable_sha256: None,
                model: "model".into(),
                configuration_digest: "config".into(),
            },
            environment_digest: "environment".into(),
            trial_index: 1,
            agent_attempts: 1,
            status: TrialStatus::Failed,
            changed_files: Vec::new(),
            patch: "<script>alert('xss')</script>".into(),
            grader: Some(GraderOutcome {
                success: false,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "assertion failed".into(),
                duration_ms: 5,
                checks: vec![GraderCheck {
                    name: "hidden regression".into(),
                    kind: GraderCheckKind::FailToPass,
                    passed: false,
                    details: "expected 42".into(),
                }],
            }),
            events: vec![AgentEvent {
                sequence: 1,
                timestamp: chrono::Utc::now(),
                kind: AgentEventKind::ToolCall,
                message: "edited source".into(),
                raw: None,
            }],
            usage: AgentUsage::default(),
            duration_ms: 10,
            termination_reason: None,
            artifacts: BTreeMap::new(),
            error: None,
        }],
        10,
    )
}

#[test]
fn repository_html_is_self_contained_and_escapes_evidence() {
    let html = generate_repository_html(&report());

    assert!(html.contains("Agent Suite"));
    assert!(html.contains("95% CI"));
    assert!(html.contains("hidden regression"));
    assert!(html.contains("&lt;script&gt;alert"));
    assert!(!html.contains("<script>alert('xss')</script>"));
    assert!(!html.contains("<script src="));
    assert!(!html.contains("<link rel=\"stylesheet\""));
}

#[test]
fn repository_sarif_contains_only_deterministic_failed_checks() {
    let sarif = generate_repository_sarif(&report()).unwrap();
    let results = &sarif["runs"][0]["results"];

    assert_eq!(results.as_array().unwrap().len(), 1);
    assert_eq!(results[0]["ruleId"], "forgetest/fail_to_pass");
    assert_eq!(
        results[0]["message"]["text"],
        "hidden regression: expected 42"
    );
}

#[test]
fn repository_html_renders_paired_bootstrap_comparisons() {
    let mut report = report();
    report.aggregate.pairwise = vec![PairwiseAgentComparison {
        agent_a: "codex/model@1#a".into(),
        agent_b: "claude/model@1#b".into(),
        common_tasks: 12,
        delta_b_minus_a: 0.125,
        ci_95_low: -0.05,
        ci_95_high: 0.25,
        bootstrap_iterations: 10_000,
    }];

    let html = generate_repository_html(&report);

    assert!(html.contains("Paired comparisons"));
    assert!(html.contains("10,000"));
    assert!(html.contains("+12.5%"));
    assert!(html.contains("-5.0% to 25.0%"));
}
