//! CLI integration tests using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn forgetest() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("forgetest").unwrap()
}

#[test]
fn validate_valid_eval_set() {
    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-basics.toml")
        .assert()
        .success()
        .stdout(predicate::str::contains("15 cases"))
        .stdout(predicate::str::contains("All eval sets valid"));
}

#[test]
fn validate_algorithms_eval_set() {
    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-algorithms.toml")
        .assert()
        .success()
        .stdout(predicate::str::contains("10 cases"));
}

#[test]
fn validate_async_eval_set() {
    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-async.toml")
        .assert()
        .success()
        .stdout(predicate::str::contains("5 cases"));
}

#[test]
fn validate_directory() {
    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg("../../eval-sets")
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust Basics"))
        .stdout(predicate::str::contains("Rust Algorithms"))
        .stdout(predicate::str::contains("Rust Async"));
}

#[test]
fn validate_nonexistent_file() {
    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg("nonexistent.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn init_creates_files() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created forgetest.toml"))
        .stdout(predicate::str::contains("Created eval-sets/example.toml"));

    assert!(dir.path().join("forgetest.toml").exists());
    assert!(dir.path().join("eval-sets/example.toml").exists());
}

#[test]
fn init_skips_existing() {
    let dir = TempDir::new().unwrap();

    // First init
    forgetest()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    // Second init should skip
    forgetest()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn compare_reports() {
    let dir = TempDir::new().unwrap();

    // Create two JSON report files
    let baseline = make_test_report("case1", "model1", true, 3, 0);
    let current = make_test_report("case1", "model1", false, 0, 0);

    let baseline_path = dir.path().join("baseline.json");
    let current_path = dir.path().join("current.json");

    std::fs::write(&baseline_path, &baseline).unwrap();
    std::fs::write(&current_path, &current).unwrap();

    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg(&baseline_path)
        .arg("--current")
        .arg(&current_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("regression"));
}

#[test]
fn compare_nonexistent_report() {
    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg("no_such_file.json")
        .arg("--current")
        .arg("also_no_file.json")
        .assert()
        .failure();
}

#[test]
fn help_output() {
    forgetest()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("LLM code-quality eval harness"));
}

#[test]
fn version_output() {
    forgetest()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("forgetest"));
}

/// Create a minimal valid JSON report for testing.
fn make_test_report(
    case_id: &str,
    model: &str,
    compile_ok: bool,
    tests_pass: u32,
    tests_fail: u32,
) -> String {
    let test_execution = if compile_ok {
        format!(
            r#"{{
                "passed": {tests_pass},
                "failed": {tests_fail},
                "ignored": 0,
                "duration_ms": 100,
                "failures": []
            }}"#,
        )
    } else {
        "null".to_string()
    };

    format!(
        r#"{{
    "id": "00000000-0000-0000-0000-000000000000",
    "created_at": "2025-01-01T00:00:00Z",
    "eval_set": {{
        "id": "test",
        "name": "Test",
        "case_count": 1
    }},
    "models_evaluated": ["{model}"],
    "results": [{{
        "case_id": "{case_id}",
        "model": "{model}",
        "provider": "test",
        "generated_code": "",
        "compilation": {{
            "success": {compile_ok},
            "errors": [],
            "warnings": [],
            "duration_ms": 100
        }},
        "test_execution": {test_execution},
        "clippy": null,
        "timing": {{
            "llm_request_ms": 100,
            "compilation_ms": 100,
            "test_execution_ms": 100,
            "total_ms": 300
        }},
        "token_usage": {{
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30,
            "estimated_cost_usd": 0.0
        }},
        "attempt": 1,
        "run_id": "00000000-0000-0000-0000-000000000000"
    }}],
    "aggregate": {{
        "per_model": {{}},
        "per_case": {{}}
    }},
    "duration_ms": 1000
}}"#
    )
}
