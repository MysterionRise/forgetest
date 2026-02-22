//! Pass@k statistical scoring and aggregate statistics.
//!
//! Implements the standard Pass@k estimator from the Codex paper (Chen et al., 2021).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::EvalSet;
use crate::results::{EvalResult, Score};

/// Compute Pass@k using the unbiased estimator.
///
/// Pass@k = 1 - C(n-c, k) / C(n, k)
///
/// Where n = total samples, c = correct samples, k = the k value.
pub fn pass_at_k(n: u32, c: u32, k: u32) -> f64 {
    if n == 0 || k == 0 {
        return 0.0;
    }
    if c == 0 {
        return 0.0;
    }
    if k > n {
        // If k > n, just return c/n as the probability
        return (c as f64 / n as f64).min(1.0);
    }
    if c >= n {
        return 1.0;
    }

    // Use log-space to avoid overflow:
    // 1 - exp(log(C(n-c, k)) - log(C(n, k)))
    // log(C(a, b)) = sum(log(a-i) for i in 0..b) - sum(log(i+1) for i in 0..b)
    let log_comb = |a: u32, b: u32| -> f64 {
        if b > a {
            return f64::NEG_INFINITY;
        }
        let mut result = 0.0f64;
        for i in 0..b {
            result += ((a - i) as f64).ln() - ((i + 1) as f64).ln();
        }
        result
    };

    let log_numerator = log_comb(n - c, k);
    let log_denominator = log_comb(n, k);

    if log_numerator == f64::NEG_INFINITY {
        return 1.0;
    }

    1.0 - (log_numerator - log_denominator).exp()
}

/// Compute Pass@k for a batch of results grouped by (case_id, model).
pub fn compute_pass_at_k_batch(
    results: &[EvalResult],
    eval_set: &EvalSet,
    k_values: &[u32],
) -> HashMap<(String, String), HashMap<u32, f64>> {
    let mut grouped: HashMap<(String, String), Vec<&EvalResult>> = HashMap::new();
    for r in results {
        grouped
            .entry((r.case_id.clone(), r.model.clone()))
            .or_default()
            .push(r);
    }

    let case_expectations: HashMap<&str, _> = eval_set
        .cases
        .iter()
        .map(|c| (c.id.as_str(), &c.expectations))
        .collect();

    let mut result = HashMap::new();
    for ((case_id, model), group) in &grouped {
        let n = group.len() as u32;
        let expectations = case_expectations.get(case_id.as_str());
        let c = group
            .iter()
            .filter(|r| {
                if let Some(exp) = expectations {
                    // "Correct" for Pass@k means: compiles AND all tests pass.
                    // Clippy warnings should NOT affect functional correctness.
                    let score = Score::compute(r, exp);
                    score.compilation >= 1.0 && score.tests >= 0.99
                } else {
                    r.compilation.success
                }
            })
            .count() as u32;

        let mut k_scores = HashMap::new();
        for &k in k_values {
            k_scores.insert(k, pass_at_k(n, c, k));
        }
        result.insert((case_id.clone(), model.clone()), k_scores);
    }

    result
}

/// Aggregate statistics across all results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    /// Per-model statistics.
    pub per_model: HashMap<String, ModelStats>,
    /// Per-case statistics.
    pub per_case: HashMap<String, CaseStats>,
}

/// Statistics for a single model across all eval cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    /// Model identifier.
    pub model: String,
    /// Pass@k scores for each k value.
    pub pass_at_k: HashMap<u32, f64>,
    /// Average compilation success rate.
    pub avg_compilation_rate: f64,
    /// Average test pass rate.
    pub avg_test_pass_rate: f64,
    /// Average clippy score.
    pub avg_clippy_score: f64,
    /// Total tokens used.
    pub total_tokens: u64,
    /// Total estimated cost in USD.
    pub total_cost_usd: f64,
    /// Average latency in milliseconds.
    pub avg_latency_ms: u64,
}

/// Statistics for a single eval case across all models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStats {
    /// Case identifier.
    pub case_id: String,
    /// Pass rate per model.
    pub per_model_pass_rate: HashMap<String, f64>,
}

/// Compute aggregate statistics from all results.
pub fn compute_aggregate_stats(
    results: &[EvalResult],
    eval_set: &EvalSet,
    k_values: &[u32],
) -> AggregateStats {
    let pass_at_k_batch = compute_pass_at_k_batch(results, eval_set, k_values);

    // Per-model stats
    let mut model_results: HashMap<String, Vec<&EvalResult>> = HashMap::new();
    for r in results {
        model_results.entry(r.model.clone()).or_default().push(r);
    }

    let case_expectations: HashMap<&str, _> = eval_set
        .cases
        .iter()
        .map(|c| (c.id.as_str(), &c.expectations))
        .collect();

    let mut per_model = HashMap::new();
    for (model, model_res) in &model_results {
        let n = model_res.len() as f64;
        let compilation_rate = model_res
            .iter()
            .filter(|r| r.compilation.success)
            .count() as f64
            / n;

        let test_pass_rate = model_res
            .iter()
            .filter_map(|r| {
                r.test_execution.as_ref().map(|t| {
                    let total = t.passed + t.failed;
                    if total == 0 {
                        0.0
                    } else {
                        t.passed as f64 / total as f64
                    }
                })
            })
            .sum::<f64>()
            / model_res
                .iter()
                .filter(|r| r.test_execution.is_some())
                .count()
                .max(1) as f64;

        let clippy_score = model_res
            .iter()
            .filter_map(|r| {
                r.clippy
                    .as_ref()
                    .map(|c| (1.0 - c.warning_count as f64 * 0.1).max(0.0))
            })
            .sum::<f64>()
            / model_res
                .iter()
                .filter(|r| r.clippy.is_some())
                .count()
                .max(1) as f64;

        let total_tokens: u64 = model_res
            .iter()
            .map(|r| r.token_usage.total_tokens as u64)
            .sum();

        let total_cost: f64 = model_res
            .iter()
            .map(|r| r.token_usage.estimated_cost_usd)
            .sum();

        let avg_latency = model_res
            .iter()
            .map(|r| r.timing.total_ms)
            .sum::<u64>()
            / model_res.len().max(1) as u64;

        // Aggregate Pass@k for this model
        let mut model_pass_k = HashMap::new();
        for &k in k_values {
            let pass_scores: Vec<f64> = pass_at_k_batch
                .iter()
                .filter(|((_, m), _)| m == model)
                .filter_map(|(_, scores)| scores.get(&k).copied())
                .collect();
            let avg = if pass_scores.is_empty() {
                0.0
            } else {
                pass_scores.iter().sum::<f64>() / pass_scores.len() as f64
            };
            model_pass_k.insert(k, avg);
        }

        per_model.insert(
            model.clone(),
            ModelStats {
                model: model.clone(),
                pass_at_k: model_pass_k,
                avg_compilation_rate: compilation_rate,
                avg_test_pass_rate: test_pass_rate,
                avg_clippy_score: clippy_score,
                total_tokens,
                total_cost_usd: total_cost,
                avg_latency_ms: avg_latency,
            },
        );
    }

    // Per-case stats
    let mut per_case = HashMap::new();
    let mut case_model_results: HashMap<String, HashMap<String, Vec<&EvalResult>>> = HashMap::new();
    for r in results {
        case_model_results
            .entry(r.case_id.clone())
            .or_default()
            .entry(r.model.clone())
            .or_default()
            .push(r);
    }

    for (case_id, model_map) in &case_model_results {
        let mut per_model_pass_rate = HashMap::new();
        for (model, res) in model_map {
            let pass_rate = res
                .iter()
                .filter(|r| {
                    if let Some(exp) = case_expectations.get(case_id.as_str()) {
                        let score = Score::compute(r, exp);
                        score.compilation >= 1.0 && score.tests >= 0.99
                    } else {
                        r.compilation.success
                    }
                })
                .count() as f64
                / res.len().max(1) as f64;
            per_model_pass_rate.insert(model.clone(), pass_rate);
        }
        per_case.insert(
            case_id.clone(),
            CaseStats {
                case_id: case_id.clone(),
                per_model_pass_rate,
            },
        );
    }

    AggregateStats {
        per_model,
        per_case,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_at_k_all_success() {
        assert!((pass_at_k(10, 10, 1) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pass_at_k_all_failure() {
        assert!((pass_at_k(10, 0, 1) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pass_at_k_half_success() {
        let score = pass_at_k(10, 5, 1);
        assert!((score - 0.5).abs() < 0.01, "expected ~0.5, got {score}");
    }

    #[test]
    fn pass_at_k_10_with_1_success() {
        // With 10 samples and 1 correct, Pass@10 should be 1.0
        let score = pass_at_k(10, 1, 10);
        assert!((score - 1.0).abs() < f64::EPSILON, "expected 1.0, got {score}");
    }

    #[test]
    fn pass_at_k_edge_k_greater_than_n() {
        let score = pass_at_k(5, 3, 10);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn pass_at_k_edge_n_zero() {
        assert_eq!(pass_at_k(0, 0, 1), 0.0);
    }
}
