//! Regression detection integration tests.
//!
//! Tests the report comparison workflow end-to-end, including
//! JSON serialization, report loading, and regression detection.

use std::collections::HashMap;

use forgetest_core::report::{EvalReport, EvalSetSummary};
use forgetest_core::results::*;
use forgetest_core::statistics::AggregateStats;
use uuid::Uuid;

fn make_result(
    case_id: &str,
    model: &str,
    compile_ok: bool,
    passed: u32,
    failed: u32,
) -> EvalResult {
    EvalResult {
        case_id: case_id.into(),
        model: model.into(),
        provider: "test".into(),
        generated_code: "// test".into(),
        compilation: CompilationResult {
            success: compile_ok,
            errors: vec![],
            warnings: vec![],
            duration_ms: 100,
        },
        test_execution: if compile_ok {
            Some(TestResult {
                passed,
                failed,
                ignored: 0,
                duration_ms: 100,
                failures: if failed > 0 {
                    vec![TestFailure {
                        name: "test_example".into(),
                        message: "assertion failed".into(),
                        stdout: String::new(),
                    }]
                } else {
                    vec![]
                },
            })
        } else {
            None
        },
        clippy: None,
        timing: TimingInfo {
            llm_request_ms: 100,
            compilation_ms: 100,
            test_execution_ms: 100,
            total_ms: 300,
        },
        token_usage: TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            estimated_cost_usd: 0.001,
        },
        attempt: 1,
        run_id: Uuid::nil(),
    }
}

fn make_report(results: Vec<EvalResult>) -> EvalReport {
    let models: Vec<String> = results
        .iter()
        .map(|r| r.model.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    EvalReport {
        id: Uuid::new_v4(),
        created_at: chrono::Utc::now(),
        eval_set: EvalSetSummary {
            id: "test-set".into(),
            name: "Test Set".into(),
            case_count: results.len(),
        },
        models_evaluated: models,
        results,
        aggregate: AggregateStats {
            per_model: HashMap::new(),
            per_case: HashMap::new(),
        },
        duration_ms: 1000,
    }
}

#[test]
fn detect_regression_when_tests_start_failing() {
    let baseline = make_report(vec![
        make_result("fib", "model-a", true, 3, 0),
        make_result("palindrome", "model-a", true, 4, 0),
    ]);

    let current = make_report(vec![
        make_result("fib", "model-a", true, 1, 2), // regression
        make_result("palindrome", "model-a", true, 4, 0), // same
    ]);

    let report = current.compare(&baseline, 0.05);

    assert!(report.has_regressions());
    assert_eq!(report.regressions.len(), 1);
    assert_eq!(report.regressions[0].case_id, "fib");
    assert!(report.regressions[0].delta < 0.0);
}

#[test]
fn detect_regression_when_compilation_breaks() {
    let baseline = make_report(vec![make_result("fib", "model-a", true, 3, 0)]);
    let current = make_report(vec![make_result("fib", "model-a", false, 0, 0)]);

    let report = current.compare(&baseline, 0.05);

    assert!(report.has_regressions());
    assert_eq!(report.regressions[0].current_score, 0.0);
}

#[test]
fn detect_improvement() {
    let baseline = make_report(vec![make_result("fib", "model-a", true, 1, 2)]);
    let current = make_report(vec![make_result("fib", "model-a", true, 3, 0)]);

    let report = current.compare(&baseline, 0.05);

    assert!(!report.has_regressions());
    assert_eq!(report.improvements.len(), 1);
    assert!(report.improvements[0].delta > 0.0);
}

#[test]
fn no_change_with_identical_results() {
    let baseline = make_report(vec![
        make_result("fib", "model-a", true, 3, 0),
        make_result("palindrome", "model-a", true, 4, 0),
    ]);

    let report = baseline.compare(&baseline, 0.05);

    assert!(!report.has_regressions());
    assert!(report.improvements.is_empty());
    assert_eq!(report.unchanged, 2);
}

#[test]
fn detect_new_and_removed_cases() {
    let baseline = make_report(vec![
        make_result("old_case", "model-a", true, 3, 0),
        make_result("shared_case", "model-a", true, 2, 0),
    ]);

    let current = make_report(vec![
        make_result("shared_case", "model-a", true, 2, 0),
        make_result("new_case", "model-a", true, 5, 0),
    ]);

    let report = current.compare(&baseline, 0.05);

    assert_eq!(report.new_cases, 1);
    assert_eq!(report.removed_cases, 1);
}

#[test]
fn multi_model_regression() {
    let baseline = make_report(vec![
        make_result("fib", "model-a", true, 3, 0),
        make_result("fib", "model-b", true, 3, 0),
    ]);

    let current = make_report(vec![
        make_result("fib", "model-a", true, 3, 0),   // same
        make_result("fib", "model-b", true, 0, 3),   // regression
    ]);

    let report = current.compare(&baseline, 0.05);

    assert_eq!(report.regressions.len(), 1);
    assert_eq!(report.regressions[0].model, "model-b");
}

#[test]
fn json_roundtrip_preserves_data() {
    let report = make_report(vec![
        make_result("fib", "model-a", true, 3, 0),
        make_result("palindrome", "model-a", false, 0, 0),
    ]);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.json");

    report.save_json(&path).unwrap();
    let loaded = EvalReport::load_json(&path).unwrap();

    assert_eq!(loaded.results.len(), 2);
    assert_eq!(loaded.eval_set.id, "test-set");
    assert_eq!(loaded.results[0].case_id, "fib");
    assert!(loaded.results[0].compilation.success);
    assert!(!loaded.results[1].compilation.success);
}

#[test]
fn markdown_report_format() {
    let baseline = make_report(vec![make_result("fib", "model-a", true, 3, 0)]);
    let current = make_report(vec![make_result("fib", "model-a", false, 0, 0)]);

    let report = current.compare(&baseline, 0.05);
    let md = report.to_markdown();

    assert!(md.contains("Regressions"));
    assert!(md.contains("fib"));
    assert!(md.contains("model-a"));
    assert!(md.contains("1 regressions"));
}

#[test]
fn threshold_controls_sensitivity() {
    let baseline = make_report(vec![make_result("fib", "model-a", true, 9, 1)]);
    // Slight change: 9/10 -> 8/10 in test pass rate
    let current = make_report(vec![make_result("fib", "model-a", true, 8, 2)]);

    // With strict threshold, this should be a regression
    let strict = current.compare(&baseline, 0.01);
    assert!(strict.has_regressions());

    // With relaxed threshold, this might not be flagged
    let relaxed = current.compare(&baseline, 0.5);
    assert!(!relaxed.has_regressions());
}
