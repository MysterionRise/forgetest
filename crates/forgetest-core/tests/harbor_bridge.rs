use std::path::Path;

use forgetest_core::harbor::{export_suite_to_harbor, import_harbor_task, HarborImportMetadata};
use forgetest_core::repository_report::GraderCheckKind;
use forgetest_core::suite::load_suite;

const IMAGE: &str =
    "forgetest-runner-rust@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn write_suite(root: &Path) {
    let task = root.join("tasks/fix");
    std::fs::create_dir_all(task.join("workspace/src")).unwrap();
    std::fs::create_dir_all(task.join("grader/tests")).unwrap();
    std::fs::write(
        root.join("suite.toml"),
        "schema_version=2\nid=\"suite\"\nname=\"Suite\"\n[[tasks]]\nid=\"fix\"\npath=\"tasks/fix\"\n",
    )
    .unwrap();
    std::fs::write(
        task.join("task.toml"),
        r#"schema_version = 1
id = "fix"
name = "Fix"
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
reference_patch = "reference.patch"
timeout_secs = 120
tags = ["rust"]
[verifier]
command = ["cargo", "test", "--all-targets", "--locked"]
timeout_secs = 60
[provenance]
kind = "authored"
license = "MIT"
"#,
    )
    .unwrap();
    std::fs::write(task.join("prompt.md"), "Fix the implementation.").unwrap();
    std::fs::write(
        task.join("workspace/Cargo.toml"),
        "[package]\nname=\"fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    std::fs::write(task.join("workspace/src/lib.rs"), "pub fn value() {}\n").unwrap();
    std::fs::write(
        task.join("grader/tests/hidden.rs"),
        "#[test] fn hidden() {}\n",
    )
    .unwrap();
    std::fs::write(
        task.join("reference.patch"),
        "diff --git a/src/lib.rs b/src/lib.rs\n",
    )
    .unwrap();
}

#[test]
fn exported_harbor_task_roundtrips_through_supported_bridge() {
    let source = tempfile::tempdir().unwrap();
    write_suite(source.path());
    let suite = load_suite(&source.path().join("suite.toml")).unwrap();
    let harbor = tempfile::tempdir().unwrap();

    export_suite_to_harbor(&suite, harbor.path(), IMAGE).unwrap();

    let harbor_task = harbor.path().join("fix");
    assert!(harbor_task.join("instruction.md").exists());
    assert!(harbor_task.join("task.toml").exists());
    assert!(harbor_task.join("environment/Dockerfile").exists());
    assert!(harbor_task.join("tests/test.sh").exists());
    assert!(std::fs::read_to_string(harbor_task.join("tests/test.sh"))
        .unwrap()
        .contains("/logs/verifier/reward.txt"));

    let imported = tempfile::tempdir().unwrap();
    import_harbor_task(
        &harbor_task,
        imported.path(),
        &HarborImportMetadata {
            suite_id: "imported".into(),
            suite_name: "Imported".into(),
            source_url: "https://example.invalid/source".into(),
            source_revision: "abc123".into(),
            license: "MIT".into(),
        },
    )
    .unwrap();
    let imported_suite = load_suite(&imported.path().join("suite.toml")).unwrap();

    assert_eq!(imported_suite.tasks.len(), 1);
    assert_eq!(imported_suite.tasks[0].prompt, "Fix the implementation.");
    assert!(imported_suite.tasks[0]
        .workspace
        .join("src/lib.rs")
        .exists());
    assert!(imported_suite.tasks[0]
        .grader
        .join("tests/hidden.rs")
        .exists());
}

#[test]
fn named_verifier_checks_survive_harbor_roundtrip() {
    let source = tempfile::tempdir().unwrap();
    write_suite(source.path());
    let task_file = source.path().join("tasks/fix/task.toml");
    let task = std::fs::read_to_string(&task_file)
        .unwrap()
        .replace(
            "[verifier]\ncommand = [\"cargo\", \"test\", \"--all-targets\", \"--locked\"]\ntimeout_secs = 60\n",
            "[verifier]\ntimeout_secs = 60\n\n[[verifier.checks]]\nname = \"hidden-tests\"\nkind = \"fail_to_pass\"\ncommand = [\"cargo\", \"test\", \"--test\", \"hidden\", \"--locked\"]\n\n[[verifier.checks]]\nname = \"clippy\"\nkind = \"clippy\"\ncommand = [\"cargo\", \"clippy\", \"--all-targets\", \"--locked\"]\n",
        );
    std::fs::write(task_file, task).unwrap();
    let suite = load_suite(&source.path().join("suite.toml")).unwrap();
    let harbor = tempfile::tempdir().unwrap();

    export_suite_to_harbor(&suite, harbor.path(), IMAGE).unwrap();
    let test_script = std::fs::read_to_string(harbor.path().join("fix/tests/test.sh")).unwrap();
    assert!(test_script.contains("'cargo' 'test' '--test' 'hidden' '--locked'"));
    assert!(test_script.contains("'cargo' 'clippy' '--all-targets' '--locked'"));

    let imported = tempfile::tempdir().unwrap();
    import_harbor_task(
        &harbor.path().join("fix"),
        imported.path(),
        &HarborImportMetadata {
            suite_id: "imported".into(),
            suite_name: "Imported".into(),
            source_url: "https://example.invalid/source".into(),
            source_revision: "abc123".into(),
            license: "MIT".into(),
        },
    )
    .unwrap();
    let imported_suite = load_suite(&imported.path().join("suite.toml")).unwrap();
    let checks = &imported_suite.tasks[0].verifier.checks;

    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].name, "hidden-tests");
    assert_eq!(checks[0].kind, GraderCheckKind::FailToPass);
    assert_eq!(checks[1].name, "clippy");
    assert_eq!(checks[1].kind, GraderCheckKind::Clippy);
}

#[test]
fn import_rejects_unmarked_harbor_task() {
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("instruction.md"), "Do work").unwrap();
    std::fs::write(
        source.path().join("task.toml"),
        "schema_version = \"1.0\"\n",
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();

    let error = import_harbor_task(
        source.path(),
        output.path(),
        &HarborImportMetadata {
            suite_id: "imported".into(),
            suite_name: "Imported".into(),
            source_url: "https://example.invalid".into(),
            source_revision: "abc".into(),
            license: "MIT".into(),
        },
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("supported forgetest bridge marker"));
}

#[test]
fn export_rejects_a_truncated_image_digest() {
    let source = tempfile::tempdir().unwrap();
    write_suite(source.path());
    let suite = load_suite(&source.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();

    let error = export_suite_to_harbor(&suite, output.path(), "runner@sha256:abc").unwrap_err();

    assert!(error.to_string().contains("complete SHA-256"));
}

#[test]
fn import_rejects_task_id_path_traversal() {
    let source = tempfile::tempdir().unwrap();
    write_suite(source.path());
    let suite = load_suite(&source.path().join("suite.toml")).unwrap();
    let harbor = tempfile::tempdir().unwrap();
    export_suite_to_harbor(&suite, harbor.path(), IMAGE).unwrap();
    let task_file = harbor.path().join("fix/task.toml");
    let content = std::fs::read_to_string(&task_file).unwrap().replace(
        "forgetest_task_id = \"fix\"",
        "forgetest_task_id = \"../escape\"",
    );
    std::fs::write(task_file, content).unwrap();
    let output = tempfile::tempdir().unwrap();

    let error = import_harbor_task(
        &harbor.path().join("fix"),
        output.path(),
        &HarborImportMetadata {
            suite_id: "imported".into(),
            suite_name: "Imported".into(),
            source_url: "https://example.invalid/source".into(),
            source_revision: "abc123".into(),
            license: "MIT".into(),
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("task ID"));
    assert!(!output.path().join("escape").exists());
}
