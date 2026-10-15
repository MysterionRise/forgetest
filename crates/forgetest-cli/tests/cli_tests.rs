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
        serde_json::from_str(&std::fs::read_to_string(&public_path).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 2);
    assert_eq!(
        raw["trials"][0]["status"],
        "passed",
        "repository demo trial failed:\n{}",
        serde_json::to_string_pretty(&raw["trials"][0]).unwrap()
    );
    assert_eq!(raw["redaction"]["redacted"], false);
    assert_eq!(public["redaction"]["redacted"], true);
    assert_eq!(public["redaction"]["rules_version"], "1");
    let public_json = std::fs::read_to_string(&public_path).unwrap();
    let public_html = std::fs::read_to_string(dir.path().join("public/report.html")).unwrap();
    let temp_dir = std::env::temp_dir();
    let canonical_temp_dir = temp_dir.canonicalize().unwrap();
    let private_paths =
        [temp_dir, canonical_temp_dir].map(|path| path.to_string_lossy().to_string());
    for public_artifact in [&public_json, &public_html] {
        for private_path in &private_paths {
            let json_escaped = serde_json::to_string(private_path).unwrap();
            let json_escaped = json_escaped.trim_matches('"');
            assert!(
                !public_artifact.contains(private_path) && !public_artifact.contains(json_escaped),
                "public evidence leaked a system temporary path"
            );
        }
        assert!(
            !public_artifact.contains("$TMPforgetest"),
            "temporary path replacement lost its separator"
        );
    }
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
fn repository_demo_passes_with_output_inside_the_cargo_workspace() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let target = workspace.join("target");
    std::fs::create_dir_all(&target).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("repository-demo-in-workspace-")
        .tempdir_in(target)
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
        .success();

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("raw/report.json")).unwrap())
            .unwrap();
    assert_eq!(
        report["trials"][0]["status"],
        "passed",
        "repository demo trial failed:\n{}",
        serde_json::to_string_pretty(&report["trials"][0]).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn repository_demo_fails_the_command_when_verification_does_not_pass() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .env("PATH", "/nonexistent")
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--runner")
        .arg("local")
        .arg("--output")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "repository demo verification failed",
        ));

    assert!(dir.path().join("raw/report.json").is_file());
    assert!(dir.path().join("public/report.json").is_file());
}

#[cfg(unix)]
#[test]
fn repository_demo_failure_points_to_emitted_html_evidence() {
    let dir = TempDir::new().unwrap();

    forgetest()
        .env("PATH", "/nonexistent")
        .arg("demo")
        .arg("--mode")
        .arg("repository")
        .arg("--runner")
        .arg("local")
        .arg("--format")
        .arg("html")
        .arg("--output")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("inspect evidence in"))
        .stderr(predicate::str::contains("report.json").not());

    assert!(dir.path().join("raw/report.html").is_file());
    assert!(dir.path().join("public/report.html").is_file());
    assert!(!dir.path().join("raw/report.json").exists());
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

#[cfg(unix)]
#[test]
fn repository_run_preflights_docker_before_invoking_an_agent() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let agent_marker = root.path().join("agent-invoked");
    let codex = bin.join("codex");
    std::fs::write(
        &codex,
        format!(
            "#!/bin/sh\nprintf invoked > '{}'\necho 'codex-cli test'\n",
            agent_marker.display()
        ),
    )
    .unwrap();
    let docker = bin.join("docker");
    std::fs::write(
        &docker,
        "#!/bin/sh\necho 'verifier image unavailable' >&2\nexit 42\n",
    )
    .unwrap();
    for executable in [&codex, &docker] {
        let mut permissions = std::fs::metadata(executable).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(executable, permissions).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    forgetest()
        .env("PATH", path)
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--profile")
        .arg("development")
        .arg("--runner")
        .arg("docker")
        .arg("--output")
        .arg(root.path().join("results"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Docker verifier preflight failed"));

    assert!(
        !agent_marker.exists(),
        "agent was invoked before Docker verifier preflight completed"
    );
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

const TEST_PRICING: &str = r#"schema_version = 1
retrieved_on = "2026-10-15"

[[models]]
model = "test-model"
input_usd_per_mtok = 2.0
cache_read_usd_per_mtok = 0.5
cache_write_usd_per_mtok = 0.0
output_usd_per_mtok = 10.0
"#;

/// Install a fake `codex` that fixes the task and reports `turn.completed` usage.
#[cfg(unix)]
fn write_usage_reporting_codex(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(bin).unwrap();
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
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100}}'
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&codex).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&codex, permissions).unwrap();
}

#[cfg(unix)]
fn path_with(bin: &Path) -> String {
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[cfg(unix)]
#[test]
fn repository_run_prices_codex_usage_and_publishes_a_verifiable_pricing_table() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    write_usage_reporting_codex(&bin);
    let pricing = root.path().join("pricing.toml");
    std::fs::write(&pricing, TEST_PRICING).unwrap();
    let output = root.path().join("results");

    forgetest()
        .env("PATH", path_with(&bin))
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--trials")
        .arg("1")
        .arg("--runner")
        .arg("local")
        .arg("--max-agent-cost-usd")
        .arg("1")
        .arg("--max-run-cost-usd")
        .arg("5")
        .arg("--pricing")
        .arg(&pricing)
        .arg("--output")
        .arg(&output)
        .arg("--format")
        .arg("all")
        .assert()
        .success()
        .stderr(predicate::str::contains("warning").not())
        .stderr(predicate::str::contains("1/1 trials passed"));

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("raw/report.json")).unwrap())
            .unwrap();
    let usage = &raw["trials"][0]["usage"];
    assert_eq!(usage["cost_source"], "pricing-table");
    assert_eq!(usage["input_tokens"], 600);
    assert_eq!(usage["cached_tokens"], 400);
    let expected = (600.0 * 2.0 + 400.0 * 0.5 + 100.0 * 10.0) / 1_000_000.0;
    assert!((usage["estimated_cost_usd"].as_f64().unwrap() - expected).abs() < 1e-12);

    let public = forgetest_core::repository_report::RepositoryReport::load_json(
        &output.join("public/report.json"),
    )
    .unwrap();
    assert!(public.redaction.redacted);
    assert!(public.policy.verify_digest());
    assert_eq!(public.policy.parameters.max_run_cost_usd, Some(5.0));
    let table = public.policy.parameters.pricing.as_ref().unwrap();
    assert_eq!(table.models[0].model, "test-model");
    assert_eq!(table.retrieved_on, "2026-10-15");
    let html = std::fs::read_to_string(output.join("public/report.html")).unwrap();
    assert!(html.contains("Pricing table"));
}

#[cfg(unix)]
#[test]
fn repository_run_development_warns_and_records_unavailable_cost() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    write_usage_reporting_codex(&bin);
    let output = root.path().join("results");

    forgetest()
        .env("PATH", path_with(&bin))
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--trials")
        .arg("1")
        .arg("--runner")
        .arg("local")
        .arg("--max-agent-cost-usd")
        .arg("1")
        .arg("--output")
        .arg(&output)
        .arg("--format")
        .arg("all")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: agent 'codex' model 'test-model' reports no vendor cost",
        ))
        .stderr(predicate::str::contains("1/1 trials passed"));

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("raw/report.json")).unwrap())
            .unwrap();
    assert_eq!(raw["trials"][0]["usage"]["cost_source"], "unavailable");
    assert_eq!(raw["trials"][0]["usage"]["input_tokens"], 600);
    let aggregate = raw["aggregate"]["per_agent"].as_object().unwrap();
    let stats = aggregate.values().next().unwrap();
    assert_eq!(stats["cost_unavailable_trials"], 1);
    let html = std::fs::read_to_string(output.join("raw/report.html")).unwrap();
    assert!(html.contains("<td>unavailable</td>"));
}

#[test]
fn repository_run_rejects_invalid_pricing_before_any_trial_without_its_path() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let pricing = root.path().join("pricing.toml");
    std::fs::write(
        &pricing,
        TEST_PRICING.replace("output_usd_per_mtok = 10.0", "output_usd_per_mtok = -10.0"),
    )
    .unwrap();
    let output = root.path().join("results");

    forgetest()
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--trials")
        .arg("1")
        .arg("--runner")
        .arg("local")
        .arg("--pricing")
        .arg(&pricing)
        .arg("--output")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains("finite and non-negative"))
        .stderr(predicate::str::contains(root.path().to_string_lossy().as_ref()).not());

    std::fs::write(&pricing, format!("{TEST_PRICING}currency = \"EUR\"\n")).unwrap();
    forgetest()
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex/test-model")
        .arg("--runner")
        .arg("local")
        .arg("--pricing")
        .arg(&pricing)
        .arg("--output")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown field"))
        .stderr(predicate::str::contains(root.path().to_string_lossy().as_ref()).not());

    assert!(!output.join("raw/trials").exists());
}

#[test]
fn repository_run_benchmark_rejects_unenforceable_cost_cap_before_docker() {
    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let lock = root.path().join("benchmark.lock.toml");
    std::fs::write(
        &lock,
        format!(
            r#"schema_version = 1
created_at = "2026-10-15T12:00:00Z"
suite_digest = "{DIGEST}"
policy_digest = "{DIGEST}"
verifier_image = "example/verifier@sha256:{DIGEST}"

[[agents]]
name = "codex"
model = "test-model"
cli_version = "codex-cli test"
executable_sha256 = "{DIGEST}"
configuration_digest = "{DIGEST}"
container_image = "example/codex@sha256:{DIGEST}"
"#
        ),
    )
    .unwrap();

    forgetest()
        .env("PATH", "/nonexistent")
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .arg("--agents")
        .arg("codex")
        .arg("--profile")
        .arg("benchmark")
        .arg("--benchmark-lock")
        .arg(&lock)
        .arg("--max-run-cost-usd")
        .arg("5")
        .arg("--output")
        .arg(root.path().join("results"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "agent 'codex' model 'test-model' reports no vendor cost and has no pricing entry",
        ));
}

#[cfg(unix)]
#[test]
fn agents_lock_rejects_unpriced_cost_cap_and_records_exact_pricing() {
    use std::os::unix::fs::PermissionsExt;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let docker_calls = root.path().join("docker-calls.log");
    let docker = bin.join("docker");
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
    let pricing = root.path().join("pricing.toml");
    std::fs::write(&pricing, TEST_PRICING).unwrap();
    let lock_command = |output: &Path| {
        let mut command = forgetest();
        command
            .env("PATH", path_with(&bin))
            .arg("agents")
            .arg("lock")
            .arg("--suite")
            .arg(root.path().join("suite.toml"))
            .arg("--agent")
            .arg(format!("codex/test-model=codex-agent@sha256:{DIGEST_A}"))
            .arg("--verifier-image")
            .arg(format!("runner@sha256:{DIGEST_B}"))
            .arg("--parallelism")
            .arg("1")
            .arg("--max-agent-cost-usd")
            .arg("1")
            .arg("--output")
            .arg(output);
        command
    };

    let rejected = root.path().join("rejected.lock.toml");
    lock_command(&rejected)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "agent 'codex' model 'test-model' reports no vendor cost and has no pricing entry",
        ));
    assert!(!rejected.exists());
    assert!(!docker_calls.exists());

    let accepted = root.path().join("benchmark.lock.toml");
    lock_command(&accepted)
        .arg("--pricing")
        .arg(&pricing)
        .arg("--max-run-cost-usd")
        .arg("5")
        .assert()
        .success()
        .stdout(predicate::str::contains("Run cost cap: USD 5"))
        .stdout(predicate::str::contains(
            "Pricing: 1 models retrieved 2026-10-15",
        ));
    let lock = forgetest_agents::BenchmarkLock::load(&accepted).unwrap();
    assert_eq!(lock.agents[0].model, "test-model");
}

const BENCHMARK_AGENT_SHA: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BENCHMARK_CARGO_SHA: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// Fake `docker` for a full benchmark run. It answers the verifier and Codex
/// image preflights, plays the Codex container against its bind-mounted
/// workspace by printing `agent_line`, and passes every grader check.
#[cfg(unix)]
fn write_benchmark_docker(bin: &Path, agent_line: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(bin).unwrap();
    let docker = bin.join("docker");
    std::fs::write(
        &docker,
        format!(
            r#"#!/bin/sh
case "$*" in
  *--interactive*)
    cat >/dev/null
    previous=""
    for argument in "$@"; do
      if [ "$previous" = "--mount" ]; then
        workspace="${{argument#type=bind,src=}}"
        workspace="${{workspace%,dst=/work}}"
      fi
      previous="$argument"
    done
    printf 'pub fn add(a: i32, b: i32) -> i32 {{ a + b }}\n' > "$workspace/src/lib.rs"
    printf '%s\n' '{agent_line}'
    ;;
  *cargo*)
    printf 'path=/usr/local/cargo\n{BENCHMARK_CARGO_SHA}  /usr/local/cargo\ncargo 1.92.0\n'
    ;;
  *)
    printf 'path=/usr/local/bin/codex\n{BENCHMARK_AGENT_SHA}  /usr/local/bin/codex\ncodex-cli test\n'
    ;;
esac
"#
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&docker).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&docker, permissions).unwrap();
}

/// Lock a priced Codex benchmark with `caps`, then run it with the same policy.
#[cfg(unix)]
fn lock_and_run_priced_benchmark(
    root: &Path,
    agent_line: &str,
    caps: &[&str],
) -> serde_json::Value {
    write_repository_suite(root);
    let bin = root.join("bin");
    write_benchmark_docker(&bin, agent_line);
    let pricing = root.join("pricing.toml");
    std::fs::write(&pricing, TEST_PRICING).unwrap();
    let lock = root.join("benchmark.lock.toml");
    forgetest()
        .env("PATH", path_with(&bin))
        .arg("agents")
        .arg("lock")
        .arg("--suite")
        .arg(root.join("suite.toml"))
        .arg("--agent")
        .arg(format!(
            "codex/test-model=codex-agent@sha256:{BENCHMARK_AGENT_SHA}"
        ))
        .arg("--verifier-image")
        .arg(format!("runner@sha256:{BENCHMARK_CARGO_SHA}"))
        .args(["--trials", "1", "--parallelism", "1"])
        .arg("--pricing")
        .arg(&pricing)
        .args(caps)
        .arg("--output")
        .arg(&lock)
        .assert()
        .success();

    let output = root.join("results");
    forgetest()
        .env("PATH", path_with(&bin))
        .arg("run")
        .arg("--suite")
        .arg(root.join("suite.toml"))
        .args(["--agents", "codex", "--profile", "benchmark"])
        .arg("--benchmark-lock")
        .arg(&lock)
        .args(["--trials", "1", "--parallelism", "1"])
        .arg("--pricing")
        .arg(&pricing)
        .args(caps)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();
    serde_json::from_str(&std::fs::read_to_string(output.join("raw/report.json")).unwrap()).unwrap()
}

#[cfg(unix)]
#[test]
fn benchmark_run_prices_codex_usage_through_the_agent_container() {
    let root = TempDir::new().unwrap();

    let report = lock_and_run_priced_benchmark(
        root.path(),
        r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100}}"#,
        &["--max-agent-cost-usd", "1", "--max-run-cost-usd", "5"],
    );

    let trial = &report["trials"][0];
    assert_eq!(trial["status"], "passed", "{trial:#}");
    assert_eq!(trial["usage"]["cost_source"], "pricing-table");
    assert_eq!(trial["usage"]["input_tokens"], 600);
    assert_eq!(report["policy"]["profile"], "benchmark");
    assert_eq!(
        report["policy"]["parameters"]["pricing"]["models"][0]["model"],
        "test-model"
    );
}

#[cfg(unix)]
#[test]
fn benchmark_run_fails_closed_when_the_agent_reports_no_usage() {
    let root = TempDir::new().unwrap();

    let report = lock_and_run_priced_benchmark(
        root.path(),
        r#"{"type":"item.completed","item":{"type":"agent_message","text":"done"}}"#,
        &["--max-agent-cost-usd", "1"],
    );

    let trial = &report["trials"][0];
    assert_eq!(trial["status"], "agent_error", "{trial:#}");
    assert_eq!(trial["termination_reason"], "budget_unverifiable");
    assert!(trial["error"]
        .as_str()
        .unwrap()
        .contains("budget unverifiable"));
    assert_eq!(trial["usage"]["cost_source"], "unavailable");
    assert!(trial["grader"].is_null());
}

#[test]
fn repository_run_rejects_invalid_run_cost_caps_before_any_trial() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let output = root.path().join("results");

    for value in ["0", "-1", "NaN", "inf"] {
        forgetest()
            .arg("run")
            .arg("--suite")
            .arg(root.path().join("suite.toml"))
            .args(["--agents", "codex/test-model", "--runner", "local"])
            .arg(format!("--max-run-cost-usd={value}"))
            .arg("--output")
            .arg(&output)
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "max-run-cost-usd must be a positive finite number",
            ));
    }
    assert!(!output.join("raw/trials").exists());
}

#[cfg(unix)]
#[test]
fn repository_run_breaker_cancels_unstarted_trials_in_raw_and_public_reports() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    write_usage_reporting_codex(&bin);
    let pricing = root.path().join("pricing.toml");
    std::fs::write(&pricing, TEST_PRICING).unwrap();
    let output = root.path().join("results");

    forgetest()
        .env("PATH", path_with(&bin))
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .args(["--agents", "codex/test-model", "--runner", "local"])
        .args(["--trials", "3", "--parallelism", "1"])
        .arg("--pricing")
        .arg(&pricing)
        .args(["--max-run-cost-usd", "0.001"])
        .arg("--output")
        .arg(&output)
        .assert()
        .success()
        .stderr(predicate::str::contains("overshoot it without bound"))
        .stderr(predicate::str::contains(
            "2 trials cancelled: run cost cap reached",
        ))
        .stderr(predicate::str::contains("1/3 trials passed"));

    for bundle in ["raw", "public"] {
        let report = forgetest_core::repository_report::RepositoryReport::load_json(
            &output.join(bundle).join("report.json"),
        )
        .unwrap();
        let statuses: Vec<_> = report.trials.iter().map(|trial| trial.status).collect();
        use forgetest_core::repository_report::TrialStatus;
        assert_eq!(
            statuses,
            vec![
                TrialStatus::Passed,
                TrialStatus::Cancelled,
                TrialStatus::Cancelled
            ],
            "{bundle}"
        );
        assert!(report.trials[1..]
            .iter()
            .all(|trial| trial.agent_attempts == 0));
        let stats = report.aggregate.per_agent.values().next().unwrap();
        assert_eq!((stats.scheduled, stats.cancelled), (3, 2));
    }
}

#[cfg(unix)]
#[test]
fn repository_run_development_warns_that_unpriced_cost_trips_the_run_breaker() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());
    let bin = root.path().join("bin");
    write_usage_reporting_codex(&bin);
    let output = root.path().join("results");

    forgetest()
        .env("PATH", path_with(&bin))
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .args(["--agents", "codex/test-model", "--runner", "local"])
        .args(["--trials", "2", "--parallelism", "1"])
        .args(["--max-agent-cost-usd", "1", "--max-run-cost-usd", "5"])
        .arg("--output")
        .arg(&output)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "the run cost breaker cancels every trial not yet started",
        ))
        .stderr(predicate::str::contains("overshoot it without bound").not())
        .stderr(predicate::str::contains(
            "1 trials cancelled: run cost unverifiable",
        ));
}

#[test]
fn repository_run_checks_caps_for_agents_without_an_explicit_model() {
    let root = TempDir::new().unwrap();
    write_repository_suite(root.path());

    forgetest()
        .arg("run")
        .arg("--suite")
        .arg(root.path().join("suite.toml"))
        .args(["--agents", "codex", "--runner", "local"])
        .args(["--max-agent-cost-usd", "1"])
        .arg("--output")
        .arg(root.path().join("results"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "warning: agent 'codex' without an explicit model reports no vendor cost",
        ))
        .stderr(predicate::str::contains("requires an explicit model"));
}
