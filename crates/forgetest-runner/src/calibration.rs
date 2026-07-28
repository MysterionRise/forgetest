//! Trusted local calibration for audited repository suites.

use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use forgetest_core::agent::{GradeCheckRequest, GradeRequest, Grader};
use forgetest_core::repository_report::GraderCheckKind;
use forgetest_core::suite::{ResolvedRepositoryTask, ResolvedSuite};
use tokio::process::Command;
use uuid::Uuid;

use crate::repository_grader::{configure_process_group, run_bounded, LocalRepositoryGrader};

/// Calibration outcome for an entire suite.
#[derive(Debug, Clone)]
pub struct SuiteCalibrationReport {
    pub tasks: Vec<TaskCalibrationResult>,
}

impl SuiteCalibrationReport {
    /// True only when every null patch fails and every reference patch passes.
    pub fn passed(&self) -> bool {
        self.tasks
            .iter()
            .all(|task| !task.null_patch_passed && task.reference_patch_passed == Some(true))
    }
}

/// Calibration outcome for one repository task.
#[derive(Debug, Clone)]
pub struct TaskCalibrationResult {
    pub task_id: String,
    pub null_patch_passed: bool,
    pub reference_patch_passed: Option<bool>,
}

/// Run local null- and reference-patch controls for a trusted suite.
pub async fn calibrate_suite(
    suite: &ResolvedSuite,
    max_output_bytes: usize,
) -> Result<SuiteCalibrationReport> {
    anyhow::ensure!(
        max_output_bytes > 0,
        "calibration output limit must be positive"
    );
    let grader = LocalRepositoryGrader::new(max_output_bytes);
    let mut tasks = Vec::with_capacity(suite.tasks.len());
    for task in &suite.tasks {
        let null_workspace =
            tempfile::tempdir().context("failed to create null-patch calibration workspace")?;
        materialize_verifier_workspace(task, null_workspace.path())?;
        let null_outcome = grader
            .grade(&grade_request(task, null_workspace.path()))
            .await
            .with_context(|| format!("null-patch grader failed for task '{}'", task.id))?;

        let reference_patch_passed = if let Some(reference_patch) = &task.reference_patch {
            let reference_workspace = tempfile::tempdir()
                .context("failed to create reference-patch calibration workspace")?;
            copy_tree(&task.workspace, reference_workspace.path(), true)?;
            apply_reference_patch(
                reference_workspace.path(),
                reference_patch,
                Duration::from_secs(task.verifier.timeout_secs),
                max_output_bytes,
            )
            .await
            .with_context(|| format!("reference patch did not apply for task '{}'", task.id))?;
            copy_tree(&task.grader, reference_workspace.path(), true)?;
            Some(
                grader
                    .grade(&grade_request(task, reference_workspace.path()))
                    .await
                    .with_context(|| {
                        format!("reference-patch grader failed for task '{}'", task.id)
                    })?
                    .success,
            )
        } else {
            None
        };

        tasks.push(TaskCalibrationResult {
            task_id: task.id.clone(),
            null_patch_passed: null_outcome.success,
            reference_patch_passed,
        });
    }
    Ok(SuiteCalibrationReport { tasks })
}

fn grade_request(task: &ResolvedRepositoryTask, workspace: &Path) -> GradeRequest {
    let checks = if task.verifier.checks.is_empty() {
        vec![GradeCheckRequest {
            name: "verifier command".into(),
            kind: GraderCheckKind::Other,
            command: task.verifier.command.clone(),
        }]
    } else {
        task.verifier
            .checks
            .iter()
            .map(|check| GradeCheckRequest {
                name: check.name.clone(),
                kind: check.kind,
                command: check.command.clone(),
            })
            .collect()
    };
    GradeRequest {
        trial_id: Uuid::new_v4(),
        workspace: workspace.to_path_buf(),
        checks,
        timeout: Duration::from_secs(task.verifier.timeout_secs),
    }
}

fn materialize_verifier_workspace(task: &ResolvedRepositoryTask, destination: &Path) -> Result<()> {
    copy_tree(&task.workspace, destination, true)?;
    copy_tree(&task.grader, destination, true)
}

async fn apply_reference_patch(
    workspace: &Path,
    patch: &Path,
    timeout: Duration,
    max_output_bytes: usize,
) -> Result<()> {
    let home = tempfile::tempdir().context("failed to create isolated Git home for calibration")?;
    let staged_patch = home.path().join("reference.patch");
    fs::copy(patch, &staged_patch).with_context(|| {
        format!(
            "failed to stage reference patch for Git: {}",
            patch.display()
        )
    })?;
    let mut command = Command::new("git");
    command
        .args(["apply", "--whitespace=nowarn"])
        .arg(staged_patch)
        .current_dir(workspace)
        .env_clear()
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    configure_process_group(&mut command);
    let output = run_bounded(command, timeout, max_output_bytes).await?;
    anyhow::ensure!(
        output.status.success(),
        "git apply failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path, overlay: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "calibration source must be a real directory: {}",
        source.display()
    );
    if !destination.exists() {
        fs::create_dir_all(destination)?;
    }
    let mut entries = fs::read_dir(source)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "calibration does not allow symlinks: {}",
            path.display()
        );
        let target = destination.join(
            path.file_name()
                .context("calibration path has no file name")?,
        );
        if metadata.is_dir() {
            copy_tree(&path, &target, overlay)?;
        } else if metadata.is_file() {
            anyhow::ensure!(
                overlay || !target.exists(),
                "calibration destination already exists: {}",
                target.display()
            );
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
            fs::set_permissions(&target, metadata.permissions())?;
        } else {
            anyhow::bail!("unsupported calibration file type: {}", path.display());
        }
    }
    Ok(())
}
