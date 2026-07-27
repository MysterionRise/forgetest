//! Eval report types with JSON persistence and regression detection.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    /// Provenance and repeatability metadata captured for this report.
    #[serde(default)]
    pub manifest: Option<RunManifest>,
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

/// Provenance and repeatability metadata for an eval run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    /// Report manifest schema version.
    pub schema_version: u32,
    /// Algorithm used by eval-set, case, and configuration identities.
    #[serde(default = "legacy_hash_algorithm")]
    pub hash_algorithm: String,
    /// forgetest package version.
    pub forgetest_version: String,
    /// Git commit SHA for the workspace, if available.
    #[serde(default)]
    pub git_sha: Option<String>,
    /// Whether tracked or untracked workspace changes were observed.
    #[serde(default)]
    pub git_dirty: Option<bool>,
    /// `rustc --version` output, if available.
    #[serde(default)]
    pub rustc_version: Option<String>,
    /// `cargo --version` output, if available.
    #[serde(default)]
    pub cargo_version: Option<String>,
    /// Runner configuration used for compile/test execution.
    pub runner: RunnerManifest,
    /// Stable hash of the eval set definition.
    pub eval_set_hash: String,
    /// Stable hashes of each eval case definition, keyed by case id.
    pub case_hashes: BTreeMap<String, String>,
    /// Models evaluated in this run.
    pub models: Vec<ModelManifest>,
    /// Pass@k values requested for this run.
    pub pass_k: Vec<u32>,
    /// Generation temperature used for this run.
    pub temperature: f64,
    /// When the manifest was created.
    pub created_at: DateTime<Utc>,
    /// Stable hash of the redacted runtime configuration.
    pub config_hash: String,
}

/// Runner metadata recorded in a report manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerManifest {
    /// Runner implementation identifier (`local` or `docker`).
    pub runner_type: String,
    /// Docker image, when the Docker runner is used.
    #[serde(default)]
    pub docker_image: Option<String>,
    /// Locally observed Docker content identity, when inspection succeeds.
    #[serde(default)]
    pub docker_image_digest: Option<String>,
}

fn legacy_hash_algorithm() -> String {
    "fnv1a64".into()
}

/// Model metadata recorded in a report manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Provider name.
    pub provider: String,
    /// Model identifier.
    pub model: String,
}

/// Compute a SHA-256 content identity for report manifests.
pub fn stable_hash_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
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
        let defaults = Expectations::default();

        // Build maps of (case_id, model) → best overall score. New reports
        // persist scores computed with the original expectations; old reports
        // fall back to the previous default-expectation behavior.
        let score_map = |report: &EvalReport| -> HashMap<(String, String), f64> {
            let mut map: HashMap<(String, String), f64> = HashMap::new();
            for r in &report.results {
                let score = r
                    .score
                    .as_ref()
                    .map(|s| s.overall)
                    .unwrap_or_else(|| Score::compute(r, &defaults).overall);
                let key = (r.case_id.clone(), r.model.clone());
                let entry = map.entry(key).or_insert(0.0);
                if score > *entry {
                    *entry = score;
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

    #[test]
    fn stable_manifest_hash_is_sha256() {
        assert_eq!(
            stable_hash_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

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
            manifest: None,
            duration_ms: 0,
        }
    }

    fn make_eval_result(
        case_id: &str,
        model: &str,
        compile_ok: bool,
        tests_pass: u32,
        tests_fail: u32,
    ) -> EvalResult {
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
            score: None,
            status: Default::default(),
            error: None,
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

    #[test]
    fn compare_prefers_stored_scores_when_present() {
        let mut baseline_result = make_eval_result("case1", "model1", true, 0, 0);
        baseline_result.score = Some(Score {
            compilation: 1.0,
            tests: 1.0,
            clippy: 1.0,
            structure: 1.0,
            overall: 1.0,
        });
        let mut current_result = make_eval_result("case1", "model1", true, 0, 0);
        current_result.score = Some(Score {
            compilation: 1.0,
            tests: 0.0,
            clippy: 1.0,
            structure: 1.0,
            overall: 0.4,
        });

        let baseline = make_report(vec![baseline_result]);
        let current = make_report(vec![current_result]);
        let report = current.compare(&baseline, 0.05);

        assert_eq!(report.regressions.len(), 1);
        assert!((report.regressions[0].baseline_score - 1.0).abs() < f64::EPSILON);
        assert!((report.regressions[0].current_score - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn old_report_json_without_manifest_or_score_still_loads() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "created_at": "2025-01-01T00:00:00Z",
            "eval_set": {"id": "test", "name": "Test", "case_count": 1},
            "models_evaluated": ["model1"],
            "results": [{
                "case_id": "case1",
                "model": "model1",
                "provider": "test",
                "generated_code": "",
                "compilation": {"success": true, "errors": [], "warnings": [], "duration_ms": 0},
                "test_execution": null,
                "clippy": null,
                "timing": {
                    "llm_request_ms": 0,
                    "compilation_ms": 0,
                    "test_execution_ms": 0,
                    "total_ms": 0
                },
                "token_usage": {
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0,
                    "estimated_cost_usd": 0.0
                },
                "attempt": 1,
                "run_id": "00000000-0000-0000-0000-000000000000"
            }],
            "aggregate": {"per_model": {}, "per_case": {}},
            "duration_ms": 0
        }"#;

        let report: EvalReport = serde_json::from_str(json).unwrap();

        assert!(report.manifest.is_none());
        assert!(report.results[0].score.is_none());
    }
}
