use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use forgetest_core::suite::{load_suite, ProvenanceKind, TaskCategory};

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval-suites/rust-agent-v1/suite.toml")
}

#[test]
fn published_corpus_has_balanced_provenance_and_categories() {
    let suite = load_suite(&corpus_path()).unwrap();

    assert_eq!(suite.tasks.len(), 12);
    assert_eq!(
        suite
            .tasks
            .iter()
            .filter(|task| task.provenance.kind == ProvenanceKind::Authored)
            .count(),
        8
    );
    assert_eq!(
        suite
            .tasks
            .iter()
            .filter(|task| task.provenance.kind == ProvenanceKind::Snapshot)
            .count(),
        4
    );
    let counts = suite
        .tasks
        .iter()
        .fold(BTreeMap::new(), |mut counts, task| {
            *counts.entry(format!("{:?}", task.category)).or_insert(0) += 1;
            counts
        });
    assert_eq!(counts[&format!("{:?}", TaskCategory::BugFix)], 3);
    assert_eq!(counts[&format!("{:?}", TaskCategory::Feature)], 3);
    assert_eq!(counts[&format!("{:?}", TaskCategory::ApiMigration)], 2);
    assert_eq!(counts[&format!("{:?}", TaskCategory::AsyncConcurrency)], 2);
    assert_eq!(
        counts[&format!("{:?}", TaskCategory::SecurityRobustness)],
        2
    );
}

#[test]
fn null_patches_fail_and_reference_patches_pass_every_task() {
    let suite = load_suite(&corpus_path()).unwrap();
    let cargo_target = tempfile::tempdir().unwrap();

    for task in &suite.tasks {
        let null_workspace = tempfile::tempdir().unwrap();
        copy_tree(&task.workspace, null_workspace.path());
        copy_tree(&task.grader, null_workspace.path());
        let fail_to_pass = task
            .verifier
            .checks
            .iter()
            .find(|check| {
                check.kind == forgetest_core::repository_report::GraderCheckKind::FailToPass
            })
            .unwrap_or_else(|| panic!("{} has no fail-to-pass check", task.id));
        let null = run(
            null_workspace.path(),
            &fail_to_pass.command,
            cargo_target.path(),
        );
        assert!(
            !null.status.success(),
            "{} null patch unexpectedly passed",
            task.id
        );

        let reference_workspace = tempfile::tempdir().unwrap();
        copy_tree(&task.workspace, reference_workspace.path());
        let patch = task
            .reference_patch
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no reference patch", task.id));
        let staged_patch = reference_workspace.path().join("reference.patch");
        std::fs::copy(patch, &staged_patch).unwrap();
        let applied = Command::new("git")
            .arg("apply")
            .arg("--whitespace=nowarn")
            .arg("reference.patch")
            .current_dir(reference_workspace.path())
            .output()
            .unwrap();
        std::fs::remove_file(staged_patch).unwrap();
        assert!(
            applied.status.success(),
            "{} reference patch did not apply:\n{}",
            task.id,
            String::from_utf8_lossy(&applied.stderr)
        );
        copy_tree(&task.grader, reference_workspace.path());
        for check in &task.verifier.checks {
            let output = run(
                reference_workspace.path(),
                &check.command,
                cargo_target.path(),
            );
            assert!(
                output.status.success(),
                "{} reference patch failed '{}':\nstdout:\n{}\nstderr:\n{}",
                task.id,
                check.name,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn run(directory: &Path, command: &[String], cargo_target: &Path) -> std::process::Output {
    Command::new(&command[0])
        .args(&command[1..])
        .current_dir(directory)
        .env("CARGO_TARGET_DIR", cargo_target)
        .output()
        .unwrap()
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
