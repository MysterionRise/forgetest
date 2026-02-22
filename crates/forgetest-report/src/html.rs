//! HTML report generator.
//!
//! Produces a self-contained HTML file with all CSS/JS inlined.

use anyhow::Result;
use std::path::Path;

use forgetest_core::report::EvalReport;

/// Escape a string for safe HTML insertion.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Generate an HTML report from an eval report.
pub fn generate_html(report: &EvalReport) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str(&format!(
        "<title>forgetest report — {}</title>\n",
        html_escape(&report.eval_set.name)
    ));
    html.push_str("<style>\n");
    html.push_str(CSS);
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    // Header
    html.push_str("<header>\n");
    html.push_str("<h1>forgetest report</h1>\n");
    html.push_str(&format!(
        "<p class=\"meta\">Eval set: <strong>{}</strong> | {} cases | {} models | {}</p>\n",
        html_escape(&report.eval_set.name),
        report.eval_set.case_count,
        report.models_evaluated.len(),
        report.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));
    html.push_str("</header>\n");

    // Summary dashboard
    html.push_str("<section class=\"dashboard\">\n");
    html.push_str("<h2>Summary</h2>\n");

    // Model summary table
    html.push_str("<table class=\"summary\">\n");
    html.push_str("<thead><tr><th>Model</th><th>Pass@1</th><th>Compile %</th><th>Test Pass %</th><th>Cost</th><th>Avg Latency</th></tr></thead>\n");
    html.push_str("<tbody>\n");
    for (model, stats) in &report.aggregate.per_model {
        let pass_1 = stats.pass_at_k.get(&1).copied().unwrap_or(0.0);
        html.push_str(&format!(
            "<tr><td>{}</td><td>{:.1}%</td><td>{:.1}%</td><td>{:.1}%</td><td>${:.4}</td><td>{}ms</td></tr>\n",
            html_escape(model),
            pass_1 * 100.0,
            stats.avg_compilation_rate * 100.0,
            stats.avg_test_pass_rate * 100.0,
            stats.total_cost_usd,
            stats.avg_latency_ms,
        ));
    }
    html.push_str("</tbody></table>\n");

    // SVG bar chart for Pass@1
    if !report.aggregate.per_model.is_empty() {
        html.push_str(&generate_bar_chart(&report.aggregate.per_model));
    }

    html.push_str("</section>\n");

    // Per-case results
    html.push_str("<section class=\"results\">\n");
    html.push_str("<h2>Results</h2>\n");
    html.push_str("<table class=\"results-table\" id=\"results\">\n");
    html.push_str("<thead><tr><th onclick=\"sortTable(0)\">Case</th><th onclick=\"sortTable(1)\">Model</th><th onclick=\"sortTable(2)\">Compile</th><th onclick=\"sortTable(3)\">Tests</th><th onclick=\"sortTable(4)\">Attempt</th></tr></thead>\n");
    html.push_str("<tbody>\n");

    for r in &report.results {
        let compile_class = if r.compilation.success {
            "pass"
        } else {
            "fail"
        };
        let compile_text = if r.compilation.success { "OK" } else { "FAIL" };

        let test_text = match &r.test_execution {
            Some(t) => format!("{}/{}", t.passed, t.passed + t.failed),
            None => "-".to_string(),
        };

        html.push_str(&format!(
            "<tr class=\"{}\"><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td></tr>\n",
            compile_class, html_escape(&r.case_id), html_escape(&r.model), compile_class, compile_text, test_text, r.attempt
        ));
    }

    html.push_str("</tbody></table>\n");
    html.push_str("</section>\n");

    // Raw JSON
    html.push_str("<section class=\"raw-data\">\n");
    html.push_str("<details>\n<summary>Raw JSON Data</summary>\n");
    html.push_str("<pre><code>");
    html.push_str(
        &serde_json::to_string_pretty(report)
            .unwrap_or_default()
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    );
    html.push_str("</code></pre>\n");
    html.push_str("</details>\n</section>\n");

    // JavaScript for sorting
    html.push_str("<script>\n");
    html.push_str(JS);
    html.push_str("</script>\n");

    html.push_str("</body>\n</html>");
    html
}

/// Write an HTML report to a file.
pub fn write_html_report(report: &EvalReport, path: &Path) -> Result<()> {
    let html = generate_html(report);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, html)?;
    Ok(())
}

fn generate_bar_chart(
    per_model: &std::collections::HashMap<String, forgetest_core::statistics::ModelStats>,
) -> String {
    let bar_height = 30;
    let max_width = 400;
    let padding = 10;
    let label_width = 200;

    let models: Vec<(&String, f64)> = per_model
        .iter()
        .map(|(m, s)| (m, s.pass_at_k.get(&1).copied().unwrap_or(0.0)))
        .collect();

    let total_height = models.len() * (bar_height + padding) + padding;

    let mut svg = format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n",
        label_width + max_width + 60,
        total_height
    );

    for (i, (model, score)) in models.iter().enumerate() {
        let y = i * (bar_height + padding) + padding;
        let width = (*score * max_width as f64) as usize;

        let color = if *score >= 0.8 {
            "#22c55e"
        } else if *score >= 0.5 {
            "#eab308"
        } else {
            "#ef4444"
        };

        svg.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\" font-size=\"14\" fill=\"currentColor\" text-anchor=\"end\" dominant-baseline=\"middle\">{}</text>\n",
            label_width - 10,
            y + bar_height / 2,
            html_escape(model)
        ));
        svg.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"4\"/>\n",
            label_width, y, width, bar_height, color
        ));
        svg.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\" font-size=\"12\" fill=\"currentColor\" dominant-baseline=\"middle\">{:.1}%</text>\n",
            label_width + width + 8,
            y + bar_height / 2,
            score * 100.0
        ));
    }

    svg.push_str("</svg>\n");
    svg
}

const CSS: &str = r#"
:root { --bg: #fff; --fg: #1a1a1a; --border: #e5e7eb; --pass: #dcfce7; --fail: #fde2e2; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #111827; --fg: #f9fafb; --border: #374151; --pass: #064e3b; --fail: #7f1d1d; }
}
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 2rem; background: var(--bg); color: var(--fg); }
h1, h2 { margin-top: 2rem; }
.meta { color: #6b7280; }
table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
th, td { border: 1px solid var(--border); padding: 0.5rem 1rem; text-align: left; }
th { background: var(--border); cursor: pointer; }
.pass { background: var(--pass); }
.fail { background: var(--fail); }
pre { overflow-x: auto; padding: 1rem; background: var(--border); border-radius: 8px; }
code { font-family: 'JetBrains Mono', 'Fira Code', monospace; font-size: 0.85rem; }
details { margin: 1rem 0; }
summary { cursor: pointer; font-weight: bold; }
svg { margin: 1rem 0; }
"#;

const JS: &str = r#"
function sortTable(col) {
  const table = document.getElementById('results');
  const tbody = table.querySelector('tbody');
  const rows = Array.from(tbody.querySelectorAll('tr'));
  const asc = table.dataset.sortCol == col && table.dataset.sortDir == 'asc' ? false : true;
  rows.sort((a, b) => {
    const va = a.cells[col].textContent;
    const vb = b.cells[col].textContent;
    return asc ? va.localeCompare(vb) : vb.localeCompare(va);
  });
  table.dataset.sortCol = col;
  table.dataset.sortDir = asc ? 'asc' : 'desc';
  rows.forEach(r => tbody.appendChild(r));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use forgetest_core::report::*;
    use forgetest_core::results::*;
    use forgetest_core::statistics::*;
    use std::collections::HashMap;

    fn make_test_report() -> EvalReport {
        EvalReport {
            id: uuid::Uuid::nil(),
            created_at: chrono::Utc::now(),
            eval_set: EvalSetSummary {
                id: "test-set".into(),
                name: "Test Set".into(),
                case_count: 1,
            },
            models_evaluated: vec!["model-1".into()],
            results: vec![EvalResult {
                case_id: "case-1".into(),
                model: "model-1".into(),
                provider: "test".into(),
                generated_code: "fn hello() {}".into(),
                compilation: CompilationResult {
                    success: true,
                    errors: vec![],
                    warnings: vec![],
                    duration_ms: 100,
                },
                test_execution: Some(TestResult {
                    passed: 3,
                    failed: 0,
                    ignored: 0,
                    duration_ms: 50,
                    failures: vec![],
                }),
                clippy: None,
                timing: TimingInfo {
                    llm_request_ms: 500,
                    compilation_ms: 100,
                    test_execution_ms: 50,
                    total_ms: 650,
                },
                token_usage: TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                    estimated_cost_usd: 0.001,
                },
                attempt: 1,
                run_id: uuid::Uuid::nil(),
            }],
            aggregate: AggregateStats {
                per_model: {
                    let mut m = HashMap::new();
                    m.insert(
                        "model-1".into(),
                        ModelStats {
                            model: "model-1".into(),
                            pass_at_k: {
                                let mut k = HashMap::new();
                                k.insert(1, 1.0);
                                k
                            },
                            avg_compilation_rate: 1.0,
                            avg_test_pass_rate: 1.0,
                            avg_clippy_score: 1.0,
                            total_tokens: 150,
                            total_cost_usd: 0.001,
                            avg_latency_ms: 650,
                        },
                    );
                    m
                },
                per_case: HashMap::new(),
            },
            duration_ms: 1000,
        }
    }

    #[test]
    fn html_report_contains_required_elements() {
        let report = make_test_report();
        let html = generate_html(&report);

        assert!(html.contains("<html"));
        assert!(html.contains("</html>"));
        assert!(html.contains("model-1"));
        assert!(html.contains("case-1"));
        assert!(html.contains("Test Set"));
    }

    #[test]
    fn html_report_write_to_file() {
        let report = make_test_report();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.html");

        write_html_report(&report, &path).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<html"));
    }
}
