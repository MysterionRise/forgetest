//! The `forgetest validate` command.

use std::path::PathBuf;

use anyhow::{Context, Result};

pub async fn execute(
    eval_set_path: Option<PathBuf>,
    suite_path: Option<PathBuf>,
    calibrate: bool,
) -> Result<()> {
    if let Some(suite_path) = suite_path {
        return validate_suite(suite_path, calibrate).await;
    }
    let eval_set_path = eval_set_path.context("--eval-set or --suite is required")?;
    let sets = if eval_set_path.is_dir() {
        forgetest_core::parser::load_eval_directory(&eval_set_path)?
    } else {
        vec![forgetest_core::parser::parse_eval_set(&eval_set_path)?]
    };

    let mut total_warnings = 0;
    let mut total_errors = 0;

    for set in &sets {
        println!("Eval set: {} ({} cases)", set.name, set.cases.len());

        let errors = forgetest_core::parser::validate_eval_set_errors(set);
        for e in &errors {
            let prefix = e
                .case_id
                .as_ref()
                .map(|id| format!("  [{id}]"))
                .unwrap_or_else(|| "  ".to_string());
            println!("{prefix} ERROR: {}", e.message);
        }
        total_errors += errors.len();

        let warnings = forgetest_core::parser::validate_eval_set(set);
        for w in &warnings {
            let prefix = w
                .case_id
                .as_ref()
                .map(|id| format!("  [{id}]"))
                .unwrap_or_else(|| "  ".to_string());
            println!("{prefix} WARNING: {}", w.message);
        }
        total_warnings += warnings.len();
    }

    if total_errors > 0 {
        println!("\n{total_errors} error(s), {total_warnings} warning(s) found.");
        anyhow::bail!("validation failed: unsupported features in eval set");
    } else if total_warnings == 0 {
        println!("All eval sets valid.");
    } else {
        println!("\n{total_warnings} warning(s) found.");
    }

    Ok(())
}

async fn validate_suite(path: PathBuf, calibrate: bool) -> Result<()> {
    let suite = forgetest_core::suite::load_suite(&path)?;
    println!(
        "Repository suite: {} ({} repository task{})",
        suite.name,
        suite.tasks.len(),
        if suite.tasks.len() == 1 { "" } else { "s" }
    );
    println!("Suite digest: {}", suite.digest);
    for task in &suite.tasks {
        println!(
            "  {} [{:?}] {} ({})",
            task.id, task.category, task.name, task.digest
        );
    }
    println!("Suite valid.");
    if calibrate {
        eprintln!("Running trusted local calibration; suite commands execute on this host.");
        let report =
            forgetest_runner::calibration::calibrate_suite(&suite, 4 * 1024 * 1024).await?;
        for task in &report.tasks {
            println!(
                "  {} - null patch: {}; reference patch: {}",
                task.task_id,
                if task.null_patch_passed {
                    "pass (INVALID)"
                } else {
                    "fail"
                },
                match task.reference_patch_passed {
                    Some(true) => "pass",
                    Some(false) => "fail (INVALID)",
                    None => "missing (INVALID)",
                }
            );
        }
        anyhow::ensure!(
            report.passed(),
            "suite calibration failed: null patches must fail and reference patches must pass"
        );
        println!("Calibration passed.");
    }
    Ok(())
}
