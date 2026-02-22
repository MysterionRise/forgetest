//! Test execution for sandboxed Cargo projects.

use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::process::Command;

use forgetest_core::results::{TestFailure, TestResult};

use crate::sandbox::Sandbox;

/// Run tests in the sandbox.
pub async fn run_tests(sandbox: &Sandbox) -> Result<TestResult> {
    let start = Instant::now();

    let mut cmd = Command::new("cargo");
    cmd.arg("test")
        .current_dir(sandbox.work_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in sandbox.build_env() {
        cmd.env(&key, &val);
    }

    let result = tokio::time::timeout(sandbox.timeout(), cmd.output())
        .await
        .context("test execution timed out")?
        .context("failed to run cargo test")?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    let combined = format!("{stdout}\n{stderr}");

    parse_test_output(&combined, duration_ms)
}

/// Parse cargo test output in the stable human-readable format.
fn parse_test_output(output: &str, duration_ms: u64) -> Result<TestResult> {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut ignored = 0u32;
    let mut failures = Vec::new();

    // Parse individual test lines
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("test ") && trimmed.ends_with(" ... ok") {
            passed += 1;
        } else if trimmed.starts_with("test ") && trimmed.ends_with(" ... FAILED") {
            failed += 1;
            let name = trimmed
                .trim_start_matches("test ")
                .trim_end_matches(" ... FAILED")
                .to_string();
            failures.push(TestFailure {
                name,
                message: String::new(),
                stdout: String::new(),
            });
        } else if trimmed.starts_with("test ") && trimmed.ends_with(" ... ignored") {
            ignored += 1;
        }
    }

    // Parse summary lines and accumulate totals across all test binaries
    // (cargo test runs unit tests, integration tests, and doc-tests separately)
    let mut found_summary = false;
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("test result:") {
            if let Some(counts) = parse_summary_line(trimmed) {
                if !found_summary {
                    // Reset from per-line counting on first summary
                    passed = 0;
                    failed = 0;
                    ignored = 0;
                    found_summary = true;
                }
                passed += counts.0;
                failed += counts.1;
                ignored += counts.2;
            }
        }
    }

    // Try to extract failure messages from the "failures:" section
    let mut in_failures = false;
    let mut current_failure_name = String::new();
    let mut current_message = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed == "failures:" {
            in_failures = true;
            continue;
        }
        if in_failures && trimmed.starts_with("---- ") && trimmed.ends_with(" stdout ----") {
            if !current_failure_name.is_empty() {
                update_failure(&mut failures, &current_failure_name, &current_message);
            }
            current_failure_name = trimmed
                .trim_start_matches("---- ")
                .trim_end_matches(" stdout ----")
                .to_string();
            current_message.clear();
            continue;
        }
        if in_failures && trimmed == "failures:" {
            // Second "failures:" section — list of failure names, stop collecting
            if !current_failure_name.is_empty() {
                update_failure(&mut failures, &current_failure_name, &current_message);
            }
            break;
        }
        if in_failures && !current_failure_name.is_empty() {
            if !current_message.is_empty() {
                current_message.push('\n');
            }
            current_message.push_str(trimmed);
        }
    }
    if !current_failure_name.is_empty() {
        update_failure(&mut failures, &current_failure_name, &current_message);
    }

    Ok(TestResult {
        passed,
        failed,
        ignored,
        duration_ms,
        failures,
    })
}

fn update_failure(failures: &mut [TestFailure], name: &str, message: &str) {
    if let Some(f) = failures.iter_mut().find(|f| f.name == name) {
        f.message = message.to_string();
    }
}

fn parse_summary_line(line: &str) -> Option<(u32, u32, u32)> {
    // "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out"
    // Split everything after "test result: " by ';' and '.'
    let after_colon = line.split(':').nth(1)?;

    let extract = |label: &str| -> u32 {
        // Split by ';' and also by '.' to separate "ok. 3 passed" into parts
        after_colon
            .split(';')
            .flat_map(|s| s.split('.'))
            .find(|s| s.trim().ends_with(label))
            .and_then(|s| {
                s.trim()
                    .strip_suffix(label)
                    .and_then(|n| n.trim().parse().ok())
            })
            .unwrap_or(0)
    };

    Some((extract("passed"), extract("failed"), extract("ignored")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_pass() {
        let output = r#"
running 3 tests
test tests::test_one ... ok
test tests::test_two ... ok
test tests::test_three ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"#;
        let result = parse_test_output(output, 100).unwrap();
        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 0);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn parse_some_failures() {
        let output = r#"
running 3 tests
test tests::test_one ... ok
test tests::test_two ... FAILED
test tests::test_three ... ok

failures:

---- tests::test_two stdout ----
thread 'tests::test_two' panicked at 'assertion `left == right` failed
  left: 1
 right: 2'

failures:
    tests::test_two

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"#;
        let result = parse_test_output(output, 100).unwrap();
        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "tests::test_two");
        assert!(result.failures[0].message.contains("assertion"));
    }

    #[test]
    fn parse_no_tests() {
        let output = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let result = parse_test_output(output, 0).unwrap();
        assert_eq!(result.passed, 0);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn parse_with_ignored() {
        let output = r#"
running 3 tests
test tests::test_one ... ok
test tests::test_two ... ignored
test tests::test_three ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
"#;
        let result = parse_test_output(output, 100).unwrap();
        assert_eq!(result.passed, 2);
        assert_eq!(result.ignored, 1);
    }
}
