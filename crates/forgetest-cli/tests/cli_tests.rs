//! CLI integration tests using assert_cmd.

use std::path::Path;

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
fn validate_repository_suite() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());

    forgetest()
        .arg("validate")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("1 repository task"))
        .stdout(predicate::str::contains("Suite valid"));
}

#[test]
fn validate_repository_suite_calibrates_null_and_reference_controls() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());

    forgetest()
        .arg("validate")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--calibrate")
        .assert()
        .success()
        .stdout(predicate::str::contains("null patch: fail"))
        .stdout(predicate::str::contains("reference patch: pass"))
        .stdout(predicate::str::contains("Calibration passed"));
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
fn compare_rejects_unknown_format_and_negative_threshold() {
    let dir = TempDir::new().unwrap();
    let report = make_test_report("case1", "model1", true, 3, 0);
    let baseline_path = dir.path().join("baseline.json");
    let current_path = dir.path().join("current.json");
    std::fs::write(&baseline_path, &report).unwrap();
    std::fs::write(&current_path, &report).unwrap();

    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg(&baseline_path)
        .arg("--current")
        .arg(&current_path)
        .arg("--format")
        .arg("xml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown comparison format"));

    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg(&baseline_path)
        .arg("--current")
        .arg(&current_path)
        .arg("--threshold=-0.1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("non-negative finite"));
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
fn compare_v2_rejects_policy_mismatch_unless_explicitly_non_gating() {
    let dir = TempDir::new().unwrap();
    forgetest()
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--output")
        .arg(dir.path().join("demo"))
        .assert()
        .success();
    let baseline = dir.path().join("baseline.json");
    let current = dir.path().join("current.json");
    let report_path = dir.path().join("demo/raw/report.json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
    std::fs::write(&baseline, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let mut changed = value;
    let mut policy: forgetest_core::repository_report::ExecutionPolicyManifest =
        serde_json::from_value(changed["policy"].clone()).unwrap();
    policy.parameters.parallelism += 1;
    changed["policy"] = serde_json::to_value(policy.sealed()).unwrap();
    std::fs::write(&current, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();

    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--current")
        .arg(&current)
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomparable"));

    forgetest()
        .arg("compare")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--current")
        .arg(&current)
        .arg("--allow-incomparable")
        .assert()
        .success()
        .stdout(predicate::str::contains("non-gating"));
}

#[test]
fn redact_command_writes_public_repository_artifacts() {
    let dir = TempDir::new().unwrap();
    forgetest()
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--output")
        .arg(dir.path().join("demo"))
        .assert()
        .success();
    let output = dir.path().join("sanitized");

    forgetest()
        .arg("redact")
        .arg("--input")
        .arg(dir.path().join("demo/raw/report.json"))
        .arg("--output")
        .arg(&output)
        .arg("--format")
        .arg("all")
        .assert()
        .success()
        .stderr(predicate::str::contains("Public artifacts"));

    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("report.json")).unwrap())
            .unwrap();
    assert_eq!(value["redaction"]["redacted"], true);
    assert!(output.join("report.html").exists());
    assert!(output.join("report.sarif").exists());
}

#[cfg(unix)]
#[test]
fn agents_doctor_reports_versions_and_hashes_without_secret_values() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for (name, version) in [("codex", "codex-cli test"), ("claude", "Claude Code test")] {
        let path = bin.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho '{version}'\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    forgetest()
        .env("PATH", path)
        .env("OPENAI_API_KEY", "sk-do-not-print-this-value")
        .arg("agents")
        .arg("doctor")
        .arg("--agents")
        .arg("codex/test-model,claude/test-model")
        .assert()
        .success()
        .stdout(predicate::str::contains("codex-cli test"))
        .stdout(predicate::str::contains("Claude Code test"))
        .stdout(predicate::str::contains("SHA-256"))
        .stdout(predicate::str::contains("OPENAI_API_KEY: available"))
        .stdout(predicate::str::contains("ANTHROPIC_API_KEY: missing"))
        .stdout(predicate::str::contains("sk-do-not-print-this-value").not());
}

#[cfg(unix)]
#[test]
fn agents_doctor_verifies_locked_agent_and_verifier_images() {
    use std::os::unix::fs::PermissionsExt;

    const AGENT_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const POLICY_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const IMAGE_SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const VERIFIER_SHA: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let root = TempDir::new().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let docker = bin.join("docker");
    std::fs::write(
        &docker,
        format!(
            r#"#!/bin/sh
case "$*" in
  *codex*)
    printf 'path=/usr/local/bin/codex\n{AGENT_SHA}  /usr/local/bin/codex\ncodex-cli test\n'
    ;;
  *)
    printf 'path=/usr/local/cargo\n{VERIFIER_SHA}  /usr/local/cargo\ncargo 1.92.0\n'
    ;;
esac
"#
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&docker).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&docker, permissions).unwrap();

    let lock = root.path().join("benchmark.lock.toml");
    let agent_image = format!("example/codex@sha256:{IMAGE_SHA}");
    let configuration_digest = forgetest_agents::profile_configuration_digest(
        &forgetest_agents::CommandProfile::codex("test-model"),
        &agent_image,
    );
    std::fs::write(
        &lock,
        format!(
            r#"schema_version = 1
created_at = "2026-07-27T12:00:00Z"
suite_digest = "{AGENT_SHA}"
policy_digest = "{POLICY_SHA}"
verifier_image = "example/verifier@sha256:{IMAGE_SHA}"

[[agents]]
name = "codex"
model = "test-model"
cli_version = "codex-cli test"
executable_sha256 = "{AGENT_SHA}"
configuration_digest = "{configuration_digest}"
container_image = "{agent_image}"
"#
        ),
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    forgetest()
        .env("PATH", path)
        .arg("agents")
        .arg("doctor")
        .arg("--benchmark-lock")
        .arg(lock)
        .assert()
        .success()
        .stdout(predicate::str::contains("codex locked image: verified"))
        .stdout(predicate::str::contains("verifier image: verified"));
}

#[cfg(unix)]
#[test]
fn agents_lock_inspects_images_and_writes_exact_policy() {
    use std::os::unix::fs::PermissionsExt;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let docker = bin.join("docker");
    let docker_calls = root.path().join("docker-calls.log");
    std::fs::write(
        &docker,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> "{}"
case "$*" in
  *cargo*)
    printf 'path=/usr/local/cargo\n{DIGEST_B}  /usr/local/cargo\ncargo 1.92.0\n'
    ;;
  *)
    printf 'path=/usr/local/bin/codex\n{DIGEST_A}  /usr/local/bin/codex\ncodex-cli test\n'
    ;;
esac
"#,
            docker_calls.display()
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&docker).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&docker, permissions).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = root.path().join("benchmark.lock.toml");

    forgetest()
        .env("PATH", path)
        .arg("agents")
        .arg("lock")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agent")
        .arg(format!("codex/test-model=codex-agent@sha256:{DIGEST_A}"))
        .arg("--effort")
        .arg("codex=high")
        .arg("--verifier-image")
        .arg(format!("runner@sha256:{DIGEST_B}"))
        .arg("--parallelism")
        .arg("1")
        .arg("--output")
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::contains("Benchmark lock"));

    let lock = forgetest_agents::BenchmarkLock::load(&output).unwrap();
    assert_eq!(lock.agents.len(), 1);
    assert_eq!(lock.agents[0].model, "test-model");
    assert_eq!(lock.agents[0].effort.as_deref(), Some("high"));
    assert_eq!(lock.agents[0].executable_sha256, DIGEST_A);
    assert_eq!(lock.policy_digest.len(), 64);
    let calls = std::fs::read_to_string(docker_calls).unwrap();
    assert!(calls.contains(&format!("runner@sha256:{DIGEST_B}")));
    assert!(calls.contains("cargo"));
}

#[test]
fn agents_lock_rejects_mutable_images_before_docker() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());

    forgetest()
        .arg("agents")
        .arg("lock")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agent")
        .arg("codex/test-model=codex-agent:latest")
        .arg("--verifier-image")
        .arg("runner:latest")
        .arg("--output")
        .arg(root.path().join("benchmark.lock.toml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("verifier image must use"));
}

#[test]
fn harbor_bridge_roundtrips_supported_rust_task() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let harbor = root.path().join("harbor");
    let imported = root.path().join("imported");

    forgetest()
        .arg("harbor")
        .arg("export")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--output")
        .arg(&harbor)
        .arg("--base-image")
        .arg("runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .assert()
        .success();

    forgetest()
        .arg("harbor")
        .arg("import")
        .arg("--task")
        .arg(harbor.join("fix-add"))
        .arg("--output")
        .arg(&imported)
        .arg("--suite-id")
        .arg("imported")
        .arg("--suite-name")
        .arg("Imported")
        .arg("--source-url")
        .arg("https://example.invalid/source")
        .arg("--source-revision")
        .arg("abc123")
        .arg("--license")
        .arg("MIT")
        .assert()
        .success();

    forgetest()
        .arg("validate")
        .arg("--suite")
        .arg(imported.join("suite.toml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Suite valid"));
}

#[test]
fn help_output() {
    forgetest()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Execution-backed Rust coding-agent evaluation",
        ));
}

#[test]
fn version_output() {
    forgetest()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("forgetest"));
}

#[test]
fn demo_writes_all_report_formats_without_api_keys() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .arg("demo")
        .arg("--output")
        .arg(dir.path())
        .arg("--format")
        .arg("all")
        .assert()
        .success()
        .stderr(predicate::str::contains("deterministic offline demo"));

    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "json")));
    assert!(files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "html")));
    assert!(files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "sarif")));

    let json_path = files
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let json = std::fs::read_to_string(json_path).unwrap();
    assert!(json.contains("\"manifest\""));
    assert!(json.contains("\"score\""));
}

#[test]
fn repository_demo_writes_private_and_redacted_evidence() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--runner")
        .arg("local")
        .arg("--output")
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "deterministic repository-agent demo",
        ));

    let raw_path = dir.path().join("raw/report.json");
    let public_path = dir.path().join("public/report.json");
    assert!(raw_path.exists());
    assert!(dir.path().join("raw/report.html").exists());
    assert!(dir.path().join("raw/report.sarif").exists());
    assert!(dir.path().join("raw/artifact-manifest.json").exists());
    assert!(public_path.exists());
    assert!(dir.path().join("public/report.html").exists());
    assert!(dir.path().join("public/report.sarif").exists());
    assert!(dir.path().join("public/artifact-manifest.json").exists());

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(raw_path).unwrap()).unwrap();
    let public: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(public_path).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 2);
    assert_eq!(
        raw["trials"][0]["status"],
        "passed",
        "repository demo trial failed:\n{}",
        serde_json::to_string_pretty(&raw["trials"][0]).unwrap()
    );
    assert_eq!(raw["redaction"]["redacted"], false);
    assert_eq!(public["redaction"]["redacted"], true);
    let raw_manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("raw/artifact-manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(raw_manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| {
            file["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("trials/") && path.ends_with("/trace.jsonl"))
        }));
}

#[test]
fn repository_demo_rejects_a_nonempty_evidence_bundle() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("raw/trials/stale")).unwrap();
    std::fs::write(
        dir.path().join("raw/trials/stale/trace.jsonl"),
        "{\"stale\":true}\n",
    )
    .unwrap();

    forgetest()
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--runner")
        .arg("local")
        .arg("--output")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("evidence directory is not empty"));
}

#[test]
fn repository_demo_rejects_invalid_format_before_creating_trials() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("invalid-format");

    forgetest()
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--runner")
        .arg("local")
        .arg("--format")
        .arg("not-a-format")
        .arg("--output")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown output format"));

    assert!(!output.join("raw/trials").exists());
}

#[test]
fn run_rejects_eval_set_and_suite_together() {
    forgetest()
        .arg("run")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-basics.toml")
        .arg("--suite")
        .arg("suite.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[cfg(unix)]
#[test]
fn repository_run_executes_external_agent_and_grades_patch() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let codex = bin.join("codex");
    std::fs::write(
        &codex,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex-cli test"
  exit 0
fi
cat >/dev/null
printf 'pub fn add(a: i32, b: i32) -> i32 { a + b }\n' > src/lib.rs
printf '{"type":"item.completed","message":"fixed src/lib.rs"}\n'
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&codex).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&codex, permissions).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = root.path().join("results");

    forgetest()
        .env("PATH", path)
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--trials")
        .arg("1")
        .arg("--profile")
        .arg("development")
        .arg("--runner")
        .arg("local")
        .arg("--output")
        .arg(&output)
        .arg("--format")
        .arg("all")
        .assert()
        .success()
        .stderr(predicate::str::contains("1/1 trials passed"));

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("raw/report.json")).unwrap())
            .unwrap();
    assert_eq!(report["trials"][0]["status"], "passed");
    assert_eq!(report["trials"][0]["agent"]["model"], "test-model");
    assert!(output.join("public/report.html").exists());
}

fn write_repository_suite(root: &Path) {
    let task = root.join("tasks/fix-add");
    std::fs::create_dir_all(task.join("workspace/src")).unwrap();
    std::fs::create_dir_all(task.join("grader/tests")).unwrap();
    std::fs::write(
        root.join("suite.toml"),
        r#"schema_version = 2
id = "cli-suite"
name = "CLI Suite"
[[tasks]]
id = "fix-add"
path = "tasks/fix-add"
"#,
    )
    .unwrap();
    std::fs::write(
        task.join("task.toml"),
        r#"schema_version = 1
id = "fix-add"
name = "Fix add"
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
reference_patch = "reference.patch"
timeout_secs = 30
[verifier]
command = ["cargo", "test", "--all-targets", "--locked"]
timeout_secs = 30
[provenance]
kind = "authored"
license = "MIT"
"#,
    )
    .unwrap();
    std::fs::write(task.join("prompt.md"), "Fix add.").unwrap();
    std::fs::write(
        task.join("workspace/Cargo.toml"),
        "[package]\nname=\"cli_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        task.join("workspace/Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 3\n\n[[package]]\nname = \"cli_fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        task.join("workspace/src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a - b }\n",
    )
    .unwrap();
    std::fs::write(
        task.join("grader/tests/hidden.rs"),
        "use cli_fixture::add;\n#[test] fn adds() { assert_eq!(add(2, 3), 5); }\n",
    )
    .unwrap();
    std::fs::write(
        task.join("reference.patch"),
        r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,1 @@
-pub fn add(a: i32, b: i32) -> i32 { a - b }
+pub fn add(a: i32, b: i32) -> i32 { a + b }
"#,
    )
    .unwrap();
}

#[test]
fn demo_accepts_explicit_local_runner() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .arg("demo")
        .arg("--runner")
        .arg("local")
        .arg("--output")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stderr(predicate::str::contains("(local runner)"));

    let json = read_first_json_report(dir.path());
    assert!(json.contains(r#""runner_type": "local""#));
    assert!(json.contains(r#""docker_image": null"#));
}

#[test]
fn demo_rejects_invalid_runner() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .arg("demo")
        .arg("--runner")
        .arg("podman")
        .arg("--output")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown runner: podman"));
}

#[test]
fn demo_docker_runner_writes_manifest_when_enabled() {
    if std::env::var("FORGETEST_DOCKER_TEST").ok().as_deref() != Some("1") {
        return;
    }

    let dir = TempDir::new().unwrap();

    forgetest()
        .arg("demo")
        .arg("--runner")
        .arg("docker")
        .arg("--output")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stderr(predicate::str::contains("(docker runner)"));

    let json = read_first_json_report(dir.path());
    assert!(json.contains(r#""runner_type": "docker""#));
    assert!(json.contains(r#""docker_image": "forgetest-runner-rust:0.1.0""#));
    assert!(json.contains(r#""avg_compilation_rate": 1.0"#));
    assert!(json.contains(r#""avg_test_pass_rate": 1.0"#));
}

#[test]
fn run_rejects_invalid_runner_before_provider_setup() {
    forgetest()
        .arg("run")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-basics.toml")
        .arg("--runner")
        .arg("podman")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown runner: podman"));
}

#[test]
fn run_docker_preflight_rejects_unsupported_dependency_before_provider_setup() {
    let dir = TempDir::new().unwrap();
    let eval_path = dir.path().join("unsupported.toml");
    std::fs::write(
        &eval_path,
        r#"
[eval_set]
id = "unsupported"
name = "Unsupported"

[[cases]]
id = "needs_reqwest"
name = "Needs reqwest"
prompt = "Write a function."
dependencies = [{ name = "reqwest", version = "0.12" }]
"#,
    )
    .unwrap();

    forgetest()
        .arg("run")
        .arg("--eval-set")
        .arg(&eval_path)
        .arg("--runner")
        .arg("docker")
        .arg("--output")
        .arg(dir.path().join("results"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Docker runner only supports bundled allowlisted dependencies in v0.1",
        ));
}

#[test]
fn run_uses_parallelism_and_temperature_from_explicit_config() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "parallelism = 0\ndefault_temperature = 3.0\n").unwrap();

    forgetest()
        .arg("run")
        .arg("--eval-set")
        .arg("../../eval-sets/rust-basics.toml")
        .arg("--config")
        .arg(config)
        .assert()
        .failure()
        .stderr(predicate::str::contains("parallelism must be at least 1"));
}

#[test]
fn validate_rejects_custom_check() {
    let dir = TempDir::new().unwrap();
    let eval_path = dir.path().join("custom.toml");
    std::fs::write(
        &eval_path,
        r#"
[eval_set]
id = "custom"
name = "Custom"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write code"

[cases.expectations]
custom_check = "grep fn"
"#,
    )
    .unwrap();

    forgetest()
        .arg("validate")
        .arg("--eval-set")
        .arg(&eval_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "custom_check is unsupported in v0.1",
        ));
}

#[tokio::test]
async fn list_models_fetches_ollama_models_dynamically() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    if std::env::var("FORGETEST_NET_TEST").ok().as_deref() != Some("1") {
        return;
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [
                {"name": "llama3.1:8b", "size": 4_000_000_000_u64}
            ]
        })))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("forgetest.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[providers.ollama]
type = "ollama"
base_url = "{}"
"#,
            server.uri()
        ),
    )
    .unwrap();

    forgetest()
        .arg("list-models")
        .arg("--provider")
        .arg("ollama")
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("llama3.1:8b"));
}

fn read_first_json_report(dir: &Path) -> String {
    let json_path = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    std::fs::read_to_string(json_path).unwrap()
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
