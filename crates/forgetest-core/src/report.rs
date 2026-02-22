//! Eval report types with JSON persistence and regression detection.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::Expectations;
use crate::results::{EvalResult, Score};
use crate::statistics::AggregateStats;

/// A complete eval report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    /// Unique report identifier.
    pub id: Uuid,
    /// When the report was created.
    pub created_at: DateTime<Utc>,
    /// Summary of the eval set.
    pub eval_set: EvalSetSummary,
    /// Models that were evaluated.
    pub models_evaluated: Vec<String>,
    /// Individual eval results.
    pub results: Vec<EvalResult>,
    /// Aggregate statistics.
    pub aggregate: AggregateStats,
    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Summary of an eval set (without the full case definitions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSetSummary {
    pub id: String,
    pub name: String,
    pub case_count: usize,
}

impl EvalReport {
    /// Save the report as JSON to a file.
    pub fn save_json(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("failed to serialize report")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
            .with_context(|| format!("failed to write report to {}", path.display()))?;
        Ok(())
    }

    /// Load a report from a JSON file.
    pub fn load_json(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read report from {}", path.display()))?;
        let report: EvalReport =
            serde_json::from_str(&content).context("failed to parse report JSON")?;
        Ok(report)
    }

    /// Compare this report against a baseline to detect regressions.
    pub fn compare(&self, baseline: &EvalReport, threshold: f64) -> RegressionReport {
        use std::collections::HashMap;

        let defaults = Expectations::default();

        // Build maps of (case_id, model) → best overall score using Score::compute
        let score_map = |report: &EvalReport| -> HashMap<(String, String), f64> {
            let mut map: HashMap<(String, String), f64> = HashMap::new();
            for r in &report.results {
                let score = Score::compute(r, &defaults);
                let key = (r.case_id.clone(), r.model.clone());
                let entry = map.entry(key).or_insert(0.0);
                if score.overall > *entry {
                    *entry = score.overall;
                }
            }
            map
        };

        let baseline_scores = score_map(baseline);
        let current_scores = score_map(self);

        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut unchanged = 0usize;
        let mut new_cases = 0usize;

        for (key, &current) in &current_scores {
            if let Some(&baseline_val) = baseline_scores.get(key) {
                let delta = current - baseline_val;
                if delta < -threshold {
                    regressions.push(Regression {
                        case_id: key.0.clone(),
                        model: key.1.clone(),
                        baseline_score: baseline_val,
                        current_score: current,
                        delta,
                    });
                } else if delta > threshold {
                    improvements.push(Improvement {
                        case_id: key.0.clone(),
                        model: key.1.clone(),
                        baseline_score: baseline_val,
                        current_score: current,
                        delta,
                    });
                } else {
                    unchanged += 1;
                }
            } else {
                new_cases += 1;
            }
        }

        let removed_cases = baseline_scores
            .keys()
            .filter(|k| !current_scores.contains_key(k))
            .count();

        RegressionReport {
            regressions,
            improvements,
            unchanged,
            new_cases,
            removed_cases,
        }
    }
}

/// Result of comparing two reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionReport {
    /// Cases where score went down.
    pub regressions: Vec<Regression>,
    /// Cases where score went up.
    pub improvements: Vec<Improvement>,
    /// Cases with no significant change.
    pub unchanged: usize,
    /// Cases in current but not baseline.
    pub new_cases: usize,
    /// Cases in baseline but not current.
    pub removed_cases: usize,
}

/// A detected regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    pub case_id: String,
    pub model: String,
    pub baseline_score: f64,
    pub current_score: f64,
    pub delta: f64,
}

/// A detected improvement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    pub case_id: String,
    pub model: String,
    pub baseline_score: f64,
    pub current_score: f64,
    pub delta: f64,
}

impl RegressionReport {
    /// Format the regression report as markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!(
            "**Summary:** {} regressions, {} improvements, {} unchanged\n\n",
            self.regressions.len(),
            self.improvements.len(),
            self.unchanged
        ));

        if !self.regressions.is_empty() {
            md.push_str("### Regressions\n\n");
            md.push_str("| Case | Model | Baseline | Current | Delta |\n");
            md.push_str("|------|-------|----------|---------|-------|\n");
            for r in &self.regressions {
                md.push_str(&format!(
                    "| {} | {} | {:.1}% | {:.1}% | {:.1}% |\n",
                    r.case_id,
                    r.model,
                    r.baseline_score * 100.0,
                    r.current_score * 100.0,
                    r.delta * 100.0
                ));
            }
            md.push('\n');
        }

        if !self.improvements.is_empty() {
            md.push_str("### Improvements\n\n");
            md.push_str("| Case | Model | Baseline | Current | Delta |\n");
            md.push_str("|------|-------|----------|---------|-------|\n");
            for i in &self.improvements {
                md.push_str(&format!(
                    "| {} | {} | {:.1}% | {:.1}% | +{:.1}% |\n",
                    i.case_id,
                    i.model,
                    i.baseline_score * 100.0,
                    i.current_score * 100.0,
                    i.delta * 100.0
                ));
            }
        }

        md
    }

    /// Returns true if there are any regressions.
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::*;
    use crate::statistics::*;
    use std::collections::HashMap;

    fn make_report(results: Vec<EvalResult>) -> EvalReport {
        let models: Vec<String> = results
            .iter()
            .map(|r| r.model.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        EvalReport {
            id: Uuid::nil(),
            created_at: Utc::now(),
            eval_set: EvalSetSummary {
                id: "test".into(),
                name: "Test".into(),
                case_count: 1,
            },
            models_evaluated: models,
            results,
            aggregate: AggregateStats {
                per_model: HashMap::new(),
                per_case: HashMap::new(),
            },
            duration_ms: 0,
        }
    }

    fn make_eval_result(case_id: &str, model: &str, compile_ok: bool, tests_pass: u32, tests_fail: u32) -> EvalResult {
        EvalResult {
            case_id: case_id.into(),
            model: model.into(),
            provider: "test".into(),
            generated_code: String::new(),
            compilation: CompilationResult {
                success: compile_ok,
                errors: vec![],
                warnings: vec![],
                duration_ms: 0,
            },
            test_execution: if compile_ok {
                Some(TestResult {
                    passed: tests_pass,
                    failed: tests_fail,
                    ignored: 0,
                    duration_ms: 0,
                    failures: vec![],
                })
            } else {
                None
            },
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
            run_id: Uuid::nil(),
        }
    }

    #[test]
    fn compare_identical_reports() {
        let r1 = make_eval_result("case1", "model1", true, 3, 0);
        let baseline = make_report(vec![r1.clone()]);
        let current = make_report(vec![r1]);

        let report = current.compare(&baseline, 0.05);
        assert!(report.regressions.is_empty());
        assert!(report.improvements.is_empty());
        assert_eq!(report.unchanged, 1);
    }

    #[test]
    fn compare_with_regression() {
        let baseline = make_report(vec![make_eval_result("case1", "model1", true, 3, 0)]);
        let current = make_report(vec![make_eval_result("case1", "model1", false, 0, 0)]);

        let report = current.compare(&baseline, 0.05);
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.regressions[0].case_id, "case1");
    }

    #[test]
    fn compare_with_new_and_removed() {
        let baseline = make_report(vec![make_eval_result("old_case", "model1", true, 1, 0)]);
        let current = make_report(vec![make_eval_result("new_case", "model1", true, 1, 0)]);

        let report = current.compare(&baseline, 0.05);
        assert_eq!(report.new_cases, 1);
        assert_eq!(report.removed_cases, 1);
    }

    #[test]
    fn json_roundtrip() {
        let report = make_report(vec![make_eval_result("case1", "model1", true, 3, 0)]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");

        report.save_json(&path).unwrap();
        let loaded = EvalReport::load_json(&path).unwrap();

        assert_eq!(loaded.eval_set.id, "test");
        assert_eq!(loaded.results.len(), 1);
    }

    #[test]
    fn markdown_output() {
        let baseline = make_report(vec![make_eval_result("case1", "model1", true, 3, 0)]);
        let current = make_report(vec![make_eval_result("case1", "model1", false, 0, 0)]);

        let report = current.compare(&baseline, 0.05);
        let md = report.to_markdown();
        assert!(md.contains("Regressions"));
        assert!(md.contains("case1"));
    }
}
