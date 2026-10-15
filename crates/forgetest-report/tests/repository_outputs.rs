use std::collections::BTreeMap;

use forgetest_core::agent::{AgentEvent, AgentEventKind, AgentIdentity, AgentUsage, CostSource};
use forgetest_core::pricing::{ModelPricing, PricingTable};
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

#[test]
fn repository_html_marks_unavailable_cost_only_when_recorded() {
    let legacy = generate_repository_html(&report());
    assert!(!legacy.contains("unavailable"));

    let mut trials = report().trials;
    trials[0].usage = AgentUsage::unavailable();
    let mut priced = trials[0].clone();
    priced.id = Uuid::new_v4();
    priced.trial_index = 2;
    priced.usage = AgentUsage {
        estimated_cost_usd: 0.5,
        cost_source: Some(CostSource::PricingTable),
        ..AgentUsage::default()
    };
    trials.push(priced);
    let base = report();
    let report = RepositoryReport::new(base.suite, base.policy, trials, 10);

    let html = generate_repository_html(&report);

    assert!(html.contains("$0.5000 (1 unavailable)"));

    let mut all_unavailable = report.trials.clone();
    all_unavailable[1].usage = AgentUsage::unavailable();
    let base = self::report();
    let html = generate_repository_html(&RepositoryReport::new(
        base.suite,
        base.policy,
        all_unavailable,
        10,
    ));
    assert!(html.contains("<td>unavailable</td>"));
    assert!(!html.contains("(2 unavailable)"));
}

#[test]
fn repository_html_shows_run_cap_and_every_pricing_row() {
    let mut report = report();
    report.policy.parameters.max_run_cost_usd = Some(7.5);
    report.policy.parameters.pricing = Some(PricingTable {
        schema_version: 1,
        retrieved_on: "2026-10-15".into(),
        models: vec![
            ModelPricing {
                model: "model".into(),
                input_usd_per_mtok: 1.25,
                cache_read_usd_per_mtok: 0.125,
                cache_write_usd_per_mtok: 0.0,
                output_usd_per_mtok: 10.0,
            },
            ModelPricing {
                model: "<other-model>".into(),
                input_usd_per_mtok: 3.0,
                cache_read_usd_per_mtok: 0.3,
                cache_write_usd_per_mtok: 3.75,
                output_usd_per_mtok: 15.0,
            },
        ],
    });

    let html = generate_repository_html(&report);

    assert!(html.contains("Run cost cap"));
    assert!(html.contains("USD 7.5000"));
    assert!(html.contains("retrieved 2026-10-15"));
    assert!(
        html.contains("<tr><td>model</td><td>1.25</td><td>0.125</td><td>0</td><td>10</td></tr>")
    );
    assert!(html.contains(
        "<tr><td>&lt;other-model&gt;</td><td>3</td><td>0.3</td><td>3.75</td><td>15</td></tr>"
    ));
}
