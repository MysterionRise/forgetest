//! The `forgetest compare` command.

use std::path::PathBuf;

use anyhow::Result;

use forgetest_core::report::EvalReport;
use forgetest_core::repository_report::{Compatibility, RepositoryReport};

pub fn execute(
    baseline_path: PathBuf,
    current_path: PathBuf,
    threshold: f64,
    fail_on_regression: bool,
    format: String,
    allow_incomparable: bool,
) -> Result<()> {
    anyhow::ensure!(
        threshold.is_finite() && threshold >= 0.0,
        "threshold must be a non-negative finite number"
    );
    anyhow::ensure!(
        matches!(format.as_str(), "text" | "json" | "markdown" | "md"),
        "unknown comparison format: {format}"
    );

    let baseline_kind = report_schema(&baseline_path)?;
    let current_kind = report_schema(&current_path)?;
    anyhow::ensure!(
        baseline_kind == current_kind,
        "reports use different schemas and cannot be compared"
    );
    if baseline_kind == ReportSchema::RepositoryV2 {
        return execute_repository(
            baseline_path,
            current_path,
            threshold,
            fail_on_regression,
            format,
            allow_incomparable,
        );
    }

    let baseline = EvalReport::load_json(&baseline_path)?;
    let current = EvalReport::load_json(&current_path)?;

    let report = current.compare(&baseline, threshold);

    match format.as_str() {
        "markdown" | "md" => {
            println!("{}", report.to_markdown());
        }
        "json" => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "text" => {
            println!(
                "Comparison: {} regressions, {} improvements, {} unchanged",
                report.regressions.len(),
                report.improvements.len(),
                report.unchanged
            );

            if !report.regressions.is_empty() {
                println!("\nRegressions:");
                for r in &report.regressions {
                    println!(
                        "  {} ({}) {:.1}% -> {:.1}% ({:+.1}%)",
                        r.case_id,
                        r.model,
                        r.baseline_score * 100.0,
                        r.current_score * 100.0,
                        r.delta * 100.0
                    );
                }
            }

            if !report.improvements.is_empty() {
                println!("\nImprovements:");
                for i in &report.improvements {
                    println!(
                        "  {} ({}) {:.1}% -> {:.1}% (+{:.1}%)",
                        i.case_id,
                        i.model,
                        i.baseline_score * 100.0,
                        i.current_score * 100.0,
                        i.delta * 100.0
                    );
                }
            }

            if report.new_cases > 0 {
                println!("\n{} new case(s)", report.new_cases);
            }
            if report.removed_cases > 0 {
                println!("{} removed case(s)", report.removed_cases);
            }
        }
        _ => unreachable!("comparison format was validated before loading reports"),
    }

    if fail_on_regression && report.has_regressions() {
        std::process::exit(1);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportSchema {
    Legacy,
    RepositoryV2,
}

fn report_schema(path: &std::path::Path) -> Result<ReportSchema> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    Ok(
        if value.get("schema_version").and_then(|value| value.as_u64()) == Some(2)
            && value.get("trials").is_some()
        {
            ReportSchema::RepositoryV2
        } else {
            ReportSchema::Legacy
        },
    )
}

fn execute_repository(
    baseline_path: PathBuf,
    current_path: PathBuf,
    threshold: f64,
    fail_on_regression: bool,
    format: String,
    allow_incomparable: bool,
) -> Result<()> {
    let baseline = RepositoryReport::load_json(&baseline_path)?;
    let current = RepositoryReport::load_json(&current_path)?;
    let comparison = current.compare(&baseline, threshold);
    if let Compatibility::Incomparable { reasons } = &comparison.compatibility {
        anyhow::ensure!(
            allow_incomparable,
            "repository reports are incomparable: {}",
            reasons.join("; ")
        );
    }

    match format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&comparison)?),
        "markdown" | "md" => {
            println!(
                "**Repository comparison:** {} regression(s), {} improvement(s), {} unchanged",
                comparison.regressions.len(),
                comparison.improvements.len(),
                comparison.unchanged.len()
            );
            if !comparison.gating_eligible {
                println!("\n**Non-gating:** suite, task, or execution policy digests differ.");
            }
            for regression in &comparison.regressions {
                println!(
                    "- Regression: `{}` {:.1}% -> {:.1}% ({:+.1}%)",
                    regression.agent,
                    regression.baseline_rate * 100.0,
                    regression.current_rate * 100.0,
                    regression.delta * 100.0
                );
            }
        }
        "text" => {
            println!(
                "Repository comparison: {} regressions, {} improvements, {} unchanged",
                comparison.regressions.len(),
                comparison.improvements.len(),
                comparison.unchanged.len()
            );
            if !comparison.gating_eligible {
                println!(
                    "This is an explicitly non-gating comparison because evidence identities differ."
                );
            }
            for regression in &comparison.regressions {
                println!(
                    "  {} {:.1}% -> {:.1}% ({:+.1}%)",
                    regression.agent,
                    regression.baseline_rate * 100.0,
                    regression.current_rate * 100.0,
                    regression.delta * 100.0
                );
            }
        }
        other => anyhow::bail!("unknown comparison format: {other}"),
    }

    if fail_on_regression && comparison.gating_eligible && comparison.has_regressions() {
        std::process::exit(1);
    }
    Ok(())
}
