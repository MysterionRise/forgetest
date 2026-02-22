//! The `forgetest compare` command.

use std::path::PathBuf;

use anyhow::Result;

use forgetest_core::report::EvalReport;

pub fn execute(
    baseline_path: PathBuf,
    current_path: PathBuf,
    threshold: f64,
    fail_on_regression: bool,
    format: String,
) -> Result<()> {
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
        _ => {
            // text format
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
    }

    if fail_on_regression && report.has_regressions() {
        std::process::exit(1);
    }

    Ok(())
}
