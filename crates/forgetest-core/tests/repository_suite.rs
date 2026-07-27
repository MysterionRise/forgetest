use std::path::Path;

use forgetest_core::suite::{load_suite, TaskCategory};

fn write_valid_suite(root: &Path) {
    std::fs::create_dir_all(root.join("tasks/fix-add/workspace/src")).unwrap();
    std::fs::create_dir_all(root.join("tasks/fix-add/grader/tests")).unwrap();
    std::fs::write(
        root.join("suite.toml"),
        r#"
schema_version = 2
id = "rust-v1"
name = "Rust v1"

[[tasks]]
id = "fix-add"
path = "tasks/fix-add"
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/task.toml"),
        r#"
schema_version = 1
id = "fix-add"
name = "Fix add"
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
reference_patch = "reference.patch"
timeout_secs = 90
tags = ["basics"]

[verifier]
command = ["cargo", "test", "--all-targets", "--locked"]
timeout_secs = 60

[provenance]
kind = "authored"
license = "MIT OR Apache-2.0"
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/prompt.md"),
        "Fix `add` without changing its API.",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/workspace/Cargo.toml"),
        "[package]\nname = \"fix-add\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/workspace/src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a - b }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/grader/tests/hidden.rs"),
        "use fix_add::add;\n#[test] fn adds() { assert_eq!(add(2, 3), 5); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix-add/reference.patch"),
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n",
    )
    .unwrap();
}

#[test]
fn loads_and_resolves_repository_suite() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());

    let suite = load_suite(&root.path().join("suite.toml")).unwrap();

    assert_eq!(suite.id, "rust-v1");
    assert_eq!(suite.tasks.len(), 1);
    assert_eq!(suite.tasks[0].id, "fix-add");
    assert_eq!(suite.tasks[0].category, TaskCategory::BugFix);
    assert_eq!(suite.tasks[0].prompt, "Fix `add` without changing its API.");
    assert!(suite.tasks[0].workspace.ends_with("workspace"));
    assert!(suite.tasks[0].grader.ends_with("grader"));
    assert_eq!(suite.digest.len(), 64);
    assert_eq!(suite.tasks[0].digest.len(), 64);
}

#[test]
fn rejects_duplicate_task_ids() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    std::fs::write(
        root.path().join("suite.toml"),
        r#"
schema_version = 2
id = "rust-v1"
name = "Rust v1"

[[tasks]]
id = "fix-add"
path = "tasks/fix-add"

[[tasks]]
id = "fix-add"
path = "tasks/fix-add"
"#,
    )
    .unwrap();

    let error = load_suite(&root.path().join("suite.toml")).unwrap_err();

    assert!(error.to_string().contains("duplicate task ID"));
}

#[test]
fn rejects_paths_outside_task_root() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    let task_path = root.path().join("tasks/fix-add/task.toml");
    let content = std::fs::read_to_string(&task_path)
        .unwrap()
        .replace("workspace = \"workspace\"", "workspace = \"../workspace\"");
    std::fs::write(task_path, content).unwrap();

    let error = load_suite(&root.path().join("suite.toml")).unwrap_err();

    assert!(error
        .to_string()
        .contains("must be a relative path without '..'"));
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_in_visible_workspace() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    symlink(
        root.path().join("tasks/fix-add/grader/tests/hidden.rs"),
        root.path().join("tasks/fix-add/workspace/hidden-link"),
    )
    .unwrap();

    let error = load_suite(&root.path().join("suite.toml")).unwrap_err();

    assert!(error.to_string().contains("symlink"));
}

#[test]
fn loads_named_fail_to_pass_and_pass_to_pass_checks() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    let task_path = root.path().join("tasks/fix-add/task.toml");
    let content = std::fs::read_to_string(&task_path).unwrap().replace(
        "command = [\"cargo\", \"test\", \"--all-targets\", \"--locked\"]\ntimeout_secs = 60",
        r#"timeout_secs = 60

[[verifier.checks]]
name = "hidden regression"
kind = "fail_to_pass"
command = ["cargo", "test", "--test", "hidden", "--locked"]

[[verifier.checks]]
name = "existing tests"
kind = "pass_to_pass"
command = ["cargo", "test", "--lib", "--locked"]"#,
    );
    std::fs::write(task_path, content).unwrap();

    let suite = load_suite(&root.path().join("suite.toml")).unwrap();

    assert_eq!(suite.tasks.len(), 1);
}

#[test]
fn rejects_mixed_legacy_command_and_named_checks() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    let task_path = root.path().join("tasks/fix-add/task.toml");
    let content = std::fs::read_to_string(&task_path).unwrap().replace(
        "timeout_secs = 60",
        r#"timeout_secs = 60

[[verifier.checks]]
name = "hidden regression"
kind = "fail_to_pass"
command = ["cargo", "test", "--test", "hidden", "--locked"]"#,
    );
    std::fs::write(task_path, content).unwrap();

    let error = load_suite(&root.path().join("suite.toml")).unwrap_err();

    assert!(error
        .to_string()
        .contains("exactly one of command or checks"));
}

#[test]
fn snapshot_provenance_requires_an_audit_date() {
    let root = tempfile::tempdir().unwrap();
    write_valid_suite(root.path());
    let task_path = root.path().join("tasks/fix-add/task.toml");
    let content = std::fs::read_to_string(&task_path).unwrap().replace(
        "kind = \"authored\"\nlicense = \"MIT OR Apache-2.0\"",
        "kind = \"snapshot\"\nlicense = \"MIT\"\nsource_url = \"https://example.invalid/repo\"\nsource_revision = \"abc123\"",
    );
    std::fs::write(task_path, content).unwrap();

    let error = load_suite(&root.path().join("suite.toml")).unwrap_err();

    assert!(error.to_string().contains("audited_at"));
}
