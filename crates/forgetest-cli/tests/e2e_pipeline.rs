//! End-to-end pipeline tests using the runner with known-good implementations.
//!
//! These tests verify that the eval pipeline (compile → test → clippy → score)
//! works correctly with both correct and broken code.

use std::time::Duration;

use forgetest_core::model::{EvalCase, Expectations, Language};
use forgetest_core::results::{Score, TokenUsage};
use forgetest_runner::{run_eval, LocalRunner};
use uuid::Uuid;

fn make_runner() -> (tempfile::TempDir, LocalRunner) {
    let target = tempfile::tempdir().unwrap();
    let runner =
        LocalRunner::new(target.path().to_path_buf()).with_timeout(Duration::from_secs(120));
    (target, runner)
}

fn make_case(id: &str, test_file: &str) -> EvalCase {
    EvalCase {
        id: id.into(),
        name: id.into(),
        description: String::new(),
        prompt: String::new(),
        language: Some(Language::Rust),
        context: vec![],
        expectations: Expectations {
            should_compile: true,
            should_pass_tests: true,
            test_file: Some(test_file.to_string()),
            expected_functions: vec![],
            ..Default::default()
        },
        tags: vec![],
        timeout_secs: Some(120),
        max_tokens: None,
    }
}

fn zero_usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        estimated_cost_usd: 0.0,
    }
}

// --- Tests with correct code ---

#[tokio::test]
async fn e2e_correct_fibonacci() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "fibonacci",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fib() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(10), 55);
    }
}
"#,
    );

    let code = r#"
pub fn fibonacci(n: u64) -> u64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0u64, 1u64);
    for _ in 2..=n {
        let tmp = a + b;
        a = b;
        b = tmp;
    }
    b
}
"#;

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    assert!(result.compilation.success, "should compile");
    let tests = result
        .test_execution
        .as_ref()
        .expect("should have test results");
    assert!(tests.passed >= 1, "should have passing tests");
    assert_eq!(tests.failed, 0, "should have no failures");

    let score = Score::compute(&result, &case.expectations);
    assert!(
        score.overall > 0.8,
        "score should be > 0.8, got {}",
        score.overall
    );
}

#[tokio::test]
async fn e2e_correct_add() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "add",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
        assert_eq!(add(0, 0), 0);
        assert_eq!(add(-5, 5), 0);
    }
}
"#,
    );

    let code = "pub fn add(a: i32, b: i32) -> i32 { a + b }";

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    assert!(result.compilation.success);
    let tests = result.test_execution.as_ref().unwrap();
    assert!(tests.passed >= 1);
    assert_eq!(tests.failed, 0);
}

// --- Tests with broken code ---

#[tokio::test]
async fn e2e_compilation_failure() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "broken_compile",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() { assert_eq!(add(1, 2), 3); }
}
"#,
    );

    // Missing semicolon
    let code = "pub fn add(a: i32, b: i32) -> i32 { a + b";

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    assert!(!result.compilation.success, "should fail to compile");
    assert!(result.test_execution.is_none(), "no tests should run");

    let score = Score::compute(&result, &case.expectations);
    assert_eq!(score.overall, 0.0, "compilation failure should score 0");
}

#[tokio::test]
async fn e2e_wrong_return_type() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "wrong_type",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() { assert_eq!(double(5), 10); }
}
"#,
    );

    // Returns String instead of i32
    let code = r#"pub fn double(n: i32) -> String { format!("{}", n * 2) }"#;

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    // Code itself compiles, but tests fail to compile due to type mismatch in assert_eq.
    // Score = 0.4 (compile) + 0.0 (tests) + 0.1 (clippy) = 0.5
    assert!(result.compilation.success, "source alone should compile");
    let score = Score::compute(&result, &case.expectations);
    assert!(
        score.overall <= 0.5,
        "wrong return type should score <= 0.5, got {}",
        score.overall
    );
}

#[tokio::test]
async fn e2e_wrong_logic() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "wrong_logic",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() { assert_eq!(add(1, 2), 3); }
    #[test]
    fn test_add_zero() { assert_eq!(add(0, 0), 0); }
}
"#,
    );

    // Bug: subtracts instead of adds
    let code = "pub fn add(a: i32, b: i32) -> i32 { a - b }";

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    assert!(result.compilation.success, "should compile");
    let tests = result
        .test_execution
        .as_ref()
        .expect("should have test results");
    assert!(tests.failed >= 1, "should have failing tests");

    let score = Score::compute(&result, &case.expectations);
    assert!(
        score.overall > 0.0 && score.overall < 0.9,
        "partial test failure should have intermediate score, got {}",
        score.overall
    );
}

#[tokio::test]
async fn e2e_empty_code() {
    let (_target, runner) = make_runner();
    let case = make_case(
        "empty",
        r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() { assert_eq!(add(1, 2), 3); }
}
"#,
    );

    // Empty code — compiles (empty lib.rs is valid Rust), but tests fail
    // because the referenced function doesn't exist.
    let code = "";

    let result = run_eval(
        &runner,
        &case,
        code,
        "mock",
        "mock",
        zero_usage(),
        0,
        1,
        Uuid::nil(),
    )
    .await
    .unwrap();

    // Empty lib.rs compiles fine; tests fail; clippy passes
    // Score = 0.4 (compile) + 0.0 (tests) + 0.1 (clippy) = 0.5
    assert!(result.compilation.success, "empty lib.rs should compile");
    let score = Score::compute(&result, &case.expectations);
    assert!(
        score.overall <= 0.5,
        "empty code should score low, got {}",
        score.overall
    );
}
