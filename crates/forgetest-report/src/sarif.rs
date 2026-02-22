//! SARIF (Static Analysis Results Interchange Format) output.
//!
//! Generates SARIF 2.1.0 documents for GitHub Code Scanning integration.

use std::path::Path;

use anyhow::Result;
use serde_json::json;

use forgetest_core::report::EvalReport;

/// Generate a SARIF 2.1.0 JSON document from an eval report.
pub fn generate_sarif(report: &EvalReport) -> serde_json::Value {
    let mut results = Vec::new();
    let mut rules = Vec::new();

    // Define rules
    let rule_defs = vec![
        (
            "compilation-failure",
            "Compilation Failure",
            "The generated code failed to compile",
        ),
        ("test-failure", "Test Failure", "One or more tests failed"),
        (
            "clippy-warning",
            "Clippy Warning",
            "Clippy reported a warning",
        ),
    ];

    for (id, name, desc) in &rule_defs {
        rules.push(json!({
            "id": id,
            "name": name,
            "shortDescription": { "text": desc },
        }));
    }

    for r in &report.results {
        let location = json!({
            "physicalLocation": {
                "artifactLocation": {
                    "uri": format!("eval-cases/{}.rs", r.case_id)
                }
            }
        });

        // Compilation failure
        if !r.compilation.success {
            let message = r
                .compilation
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "compilation failed".into());

            results.push(json!({
                "ruleId": "compilation-failure",
                "level": "error",
                "message": { "text": format!("[{}] {}: {}", r.model, r.case_id, message) },
                "locations": [location.clone()]
            }));
        }

        // Test failures
        if let Some(test) = &r.test_execution {
            for failure in &test.failures {
                results.push(json!({
                    "ruleId": "test-failure",
                    "level": "warning",
                    "message": { "text": format!("[{}] {}: test '{}' failed: {}", r.model, r.case_id, failure.name, failure.message) },
                    "locations": [location.clone()]
                }));
            }
        }

        // Clippy warnings
        if let Some(clippy) = &r.clippy {
            for warning in &clippy.warnings {
                results.push(json!({
                    "ruleId": "clippy-warning",
                    "level": "note",
                    "message": { "text": format!("[{}] {}: {}", r.model, r.case_id, warning.message) },
                    "locations": [location.clone()]
                }));
            }
        }
    }

    json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "forgetest",
                    "version": "0.1.0",
                    "informationUri": "https://github.com/MysterionRise/forgetest",
                    "rules": rules
                }
            },
            "results": results
        }]
    })
}

/// Write a SARIF report to a file.
pub fn write_sarif_report(report: &EvalReport, path: &Path) -> Result<()> {
    let sarif = generate_sarif(report);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&sarif)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgetest_core::report::*;
    use forgetest_core::results::*;
    use forgetest_core::statistics::*;
    use std::collections::HashMap;

    #[test]
    fn sarif_structure_valid() {
        let report = EvalReport {
            id: uuid::Uuid::nil(),
            created_at: chrono::Utc::now(),
            eval_set: EvalSetSummary {
                id: "test".into(),
                name: "Test".into(),
                case_count: 1,
            },
            models_evaluated: vec!["model-1".into()],
            results: vec![EvalResult {
                case_id: "case-1".into(),
                model: "model-1".into(),
                provider: "test".into(),
                generated_code: String::new(),
                compilation: CompilationResult {
                    success: false,
                    errors: vec![CompilerDiagnostic {
                        level: DiagnosticLevel::Error,
                        message: "type mismatch".into(),
                        code: Some("E0308".into()),
                        spans: vec![],
                    }],
                    warnings: vec![],
                    duration_ms: 0,
                },
                test_execution: None,
                clippy: None,
                timing: TimingInfo {
                    llm_request_ms: 0,
                    compilation_ms: 0,
                    test_execution_ms: 0,
                    total_ms: 0,
                },
                token_usage: TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    estimated_cost_usd: 0.0,
                },
                attempt: 1,
                run_id: uuid::Uuid::nil(),
            }],
            aggregate: AggregateStats {
                per_model: HashMap::new(),
                per_case: HashMap::new(),
            },
            duration_ms: 0,
        };

        let sarif = generate_sarif(&report);

        // Verify SARIF structure
        assert_eq!(sarif["version"], "2.1.0");
        assert!(sarif["runs"].is_array());
        assert!(sarif["runs"][0]["tool"]["driver"]["name"] == "forgetest");
        assert!(sarif["runs"][0]["results"].is_array());

        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["ruleId"], "compilation-failure");
        assert_eq!(results[0]["level"], "error");
    }
}
