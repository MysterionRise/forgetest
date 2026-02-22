//! Custom scorer example — compute custom scores from eval results.
//!
//! This example shows how to load an eval report and compute custom
//! scores using the forgetest library.
//!
//! ```bash
//! cargo run --example custom_scorer -- results/report.json
//! ```

use std::env;

use forgetest_core::report::EvalReport;
use forgetest_core::results::Score;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let report_path = args
        .get(1)
        .expect("Usage: custom_scorer <report.json>");

    // Load a previously generated report
    let report = EvalReport::load_json(report_path.as_ref())?;
    println!("Loaded report: {} results", report.results.len());

    // Custom scoring: weight tests more heavily
    println!("\n--- Custom Scoring (70% tests, 25% compile, 5% clippy) ---\n");
    println!("{:<30} {:<15} {:<10} {:<10}", "Case", "Model", "Default", "Custom");
    println!("{}", "-".repeat(65));

    for result in &report.results {
        // Default scoring
        let default_expectations = forgetest_core::model::Expectations::default();
        let default_score = Score::compute(result, &default_expectations);

        // Custom scoring: change the weights
        let custom_score = if !result.compilation.success {
            0.0
        } else {
            let compile = 1.0;
            let test = result
                .test_execution
                .as_ref()
                .map(|t| {
                    let total = t.passed + t.failed;
                    if total == 0 { 0.0 } else { t.passed as f64 / total as f64 }
                })
                .unwrap_or(0.0);
            let clippy = result
                .clippy
                .as_ref()
                .map(|c| (1.0 - c.warning_count as f64 * 0.1).max(0.0))
                .unwrap_or(1.0);

            // Custom weights: 25% compile, 70% tests, 5% clippy
            compile * 0.25 + test * 0.70 + clippy * 0.05
        };

        println!(
            "{:<30} {:<15} {:<10.1}% {:<10.1}%",
            result.case_id,
            result.model,
            default_score.overall * 100.0,
            custom_score * 100.0,
        );
    }

    // Compute aggregate custom score per model
    println!("\n--- Per-Model Custom Averages ---\n");
    let mut model_scores: std::collections::HashMap<&str, Vec<f64>> =
        std::collections::HashMap::new();

    for result in &report.results {
        let custom = if !result.compilation.success {
            0.0
        } else {
            let test = result
                .test_execution
                .as_ref()
                .map(|t| {
                    let total = t.passed + t.failed;
                    if total == 0 { 0.0 } else { t.passed as f64 / total as f64 }
                })
                .unwrap_or(0.0);
            let clippy = result
                .clippy
                .as_ref()
                .map(|c| (1.0 - c.warning_count as f64 * 0.1).max(0.0))
                .unwrap_or(1.0);
            0.25 + test * 0.70 + clippy * 0.05
        };
        model_scores.entry(&result.model).or_default().push(custom);
    }

    for (model, scores) in &model_scores {
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        println!("  {model}: {:.1}% average custom score", avg * 100.0);
    }

    Ok(())
}
