//! Central eval engine orchestrator.
//!
//! Coordinates multiple eval cases across multiple models with parallelism,
//! retries, and Pass@k support.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::model::{EvalSet, Language};
use crate::report::{EvalReport, EvalSetSummary};
use crate::results::{EvalResult, TimingInfo};
use crate::statistics::compute_aggregate_stats;
use crate::traits::{
    ClippyRequest, CodeRunner, CompileRequest, GenerateRequest, LlmProvider, TestRequest,
};

/// Configuration for the eval engine.
#[derive(Debug, Clone)]
pub struct EvalEngineConfig {
    /// Maximum concurrent evals.
    pub parallelism: usize,
    /// Pass@k values to compute (e.g. [1, 5, 10]).
    pub pass_k: Vec<u32>,
    /// Temperature for generation.
    pub temperature: f64,
    /// Max tokens for generation.
    pub max_tokens: u32,
    /// Retries on provider errors (not code failures).
    pub max_retries_per_case: u32,
    /// Delay between retries.
    pub retry_delay: Duration,
    /// Optional system prompt override.
    pub system_prompt_override: Option<String>,
}

impl Default for EvalEngineConfig {
    fn default() -> Self {
        Self {
            parallelism: 4,
            pass_k: vec![1],
            temperature: 0.0,
            max_tokens: 4096,
            max_retries_per_case: 3,
            retry_delay: Duration::from_secs(1),
            system_prompt_override: None,
        }
    }
}

/// Which model to evaluate.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Provider name (e.g. "anthropic").
    pub provider: String,
    /// Model identifier (e.g. "claude-sonnet-4-20250514").
    pub model: String,
}

/// Progress reporting trait.
pub trait ProgressReporter: Send + Sync {
    fn on_eval_start(&self, case_id: &str, model: &str, attempt: u32);
    fn on_eval_complete(&self, result: &EvalResult);
    fn on_eval_error(&self, case_id: &str, model: &str, error: &str);
    fn on_set_complete(&self, total: usize, completed: usize, failed: usize, elapsed: Duration);
}

/// No-op progress reporter.
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn on_eval_start(&self, _: &str, _: &str, _: u32) {}
    fn on_eval_complete(&self, _: &EvalResult) {}
    fn on_eval_error(&self, _: &str, _: &str, _: &str) {}
    fn on_set_complete(&self, _: usize, _: usize, _: usize, _: Duration) {}
}

/// The central eval engine.
pub struct EvalEngine {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    runner: Arc<dyn CodeRunner>,
    config: EvalEngineConfig,
}

impl EvalEngine {
    pub fn new(
        providers: HashMap<String, Arc<dyn LlmProvider>>,
        runner: Arc<dyn CodeRunner>,
        config: EvalEngineConfig,
    ) -> Self {
        Self {
            providers,
            runner,
            config,
        }
    }

    /// Run evaluations for an eval set against specified models.
    pub async fn run(
        &self,
        eval_set: &EvalSet,
        models: &[ModelSpec],
        progress: &dyn ProgressReporter,
    ) -> Result<EvalReport> {
        let start = Instant::now();
        let run_id = Uuid::new_v4();
        let semaphore = Arc::new(Semaphore::new(self.config.parallelism));
        let max_k = self.config.pass_k.iter().copied().max().unwrap_or(1);

        let mut futures = FuturesUnordered::new();

        for model_spec in models {
            let Some(provider) = self.providers.get(&model_spec.provider) else {
                tracing::warn!("provider '{}' not found, skipping", model_spec.provider);
                continue;
            };

            for case in &eval_set.cases {
                for attempt in 1..=max_k {
                    let provider = Arc::clone(provider);
                    let runner = Arc::clone(&self.runner);
                    let semaphore = Arc::clone(&semaphore);
                    let case = case.clone();
                    let model = model_spec.model.clone();
                    let provider_name = model_spec.provider.clone();
                    let config = self.config.clone();

                    futures.push(async move {
                        let ctx_case_id = case.id.clone();
                        let ctx_model = model.clone();
                        let inner = async move {
                            let _permit = semaphore
                                .clone()
                                .acquire_owned()
                                .await
                                .map_err(|_| anyhow::anyhow!("semaphore closed"))?;

                            let request = GenerateRequest {
                                model: model.clone(),
                                prompt: case.prompt.clone(),
                                system_prompt: config.system_prompt_override.clone(),
                                context_files: case.context.clone(),
                                max_tokens: case.max_tokens.unwrap_or(config.max_tokens),
                                temperature: config.temperature,
                                stop_sequences: vec![],
                            };

                            let gen_start = Instant::now();

                            // Retry on transient provider errors with exponential backoff
                            let mut last_error = None;
                            let mut retry_delay = config.retry_delay;
                            for retry in 0..=config.max_retries_per_case {
                                if retry > 0 {
                                    tokio::time::sleep(retry_delay).await;
                                    retry_delay = (retry_delay * 2).min(Duration::from_secs(60));
                                }
                                match provider.generate(&request).await {
                                    Ok(response) => {
                                        let llm_ms = gen_start.elapsed().as_millis() as u64;
                                        let generated_code = response.extracted_code.clone();
                                        let language = case.language.unwrap_or(Language::Rust);
                                        let timeout_secs = case.timeout_secs.unwrap_or(60);

                                        // Compile the generated code
                                        let compile_result = runner
                                            .compile(&CompileRequest {
                                                code: generated_code.clone(),
                                                language,
                                                dependencies: vec![],
                                                timeout_secs,
                                            })
                                            .await?;
                                        let compilation_ms = compile_result.duration_ms;

                                        // Run tests if compilation succeeded and test_file is provided
                                        let test_execution = if compile_result.success
                                            && case.expectations.should_pass_tests
                                        {
                                            if let Some(test_file) = &case.expectations.test_file {
                                                Some(
                                                    runner
                                                        .run_tests(&TestRequest {
                                                            code: generated_code.clone(),
                                                            test_code: test_file.clone(),
                                                            language,
                                                            dependencies: vec![],
                                                            timeout_secs,
                                                        })
                                                        .await?,
                                                )
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };
                                        let test_execution_ms = test_execution
                                            .as_ref()
                                            .map(|t| t.duration_ms)
                                            .unwrap_or(0);

                                        // Run clippy if compilation succeeded
                                        let clippy = if compile_result.success {
                                            Some(
                                                runner
                                                    .run_clippy(&ClippyRequest {
                                                        code: generated_code.clone(),
                                                        language,
                                                        dependencies: vec![],
                                                        timeout_secs,
                                                    })
                                                    .await?,
                                            )
                                        } else {
                                            None
                                        };

                                        let total_ms = llm_ms + compilation_ms + test_execution_ms;

                                        return Ok(EvalResult {
                                            case_id: case.id.clone(),
                                            model: model.clone(),
                                            provider: provider_name.clone(),
                                            generated_code,
                                            compilation: compile_result,
                                            test_execution,
                                            clippy,
                                            timing: TimingInfo {
                                                llm_request_ms: llm_ms,
                                                compilation_ms,
                                                test_execution_ms,
                                                total_ms,
                                            },
                                            token_usage: response.token_usage,
                                            attempt,
                                            run_id,
                                        });
                                    }
                                    Err(e) => {
                                        // Check if the error is permanent (should not retry)
                                        let err_str = e.to_string();
                                        if err_str.contains("authentication")
                                            || err_str.contains("model not found")
                                        {
                                            return Err(e);
                                        }
                                        // Use provider's retry-after hint if available
                                        if err_str.contains("rate limited") {
                                            if let Some(ms) = parse_retry_after_ms(&err_str) {
                                                retry_delay = Duration::from_millis(ms);
                                            }
                                        }
                                        last_error = Some(e);
                                    }
                                }
                            }

                            Err(last_error.unwrap_or_else(|| anyhow::anyhow!("unknown error")))
                        };
                        (ctx_case_id, ctx_model, inner.await)
                    });
                }
            }
        }

        let mut results = Vec::new();
        let mut completed = 0usize;
        let mut failed = 0usize;
        let total = futures.len();

        while let Some((case_id, model, result)) = futures.next().await {
            match result {
                Ok(eval_result) => {
                    progress.on_eval_complete(&eval_result);
                    results.push(eval_result);
                    completed += 1;
                }
                Err(e) => {
                    tracing::error!("eval failed for {case_id}/{model}: {e:#}");
                    progress.on_eval_error(&case_id, &model, &e.to_string());
                    failed += 1;
                }
            }
        }

        let elapsed = start.elapsed();
        progress.on_set_complete(total, completed, failed, elapsed);

        let aggregate = compute_aggregate_stats(&results, eval_set, &self.config.pass_k);

        let models_evaluated: Vec<String> = models.iter().map(|m| m.model.clone()).collect();

        Ok(EvalReport {
            id: run_id,
            created_at: chrono::Utc::now(),
            eval_set: EvalSetSummary {
                id: eval_set.id.clone(),
                name: eval_set.name.clone(),
                case_count: eval_set.cases.len(),
            },
            models_evaluated,
            results,
            aggregate,
            duration_ms: elapsed.as_millis() as u64,
        })
    }
}

/// Parse retry-after milliseconds from a ProviderError::RateLimited message.
fn parse_retry_after_ms(err_msg: &str) -> Option<u64> {
    // Error format: "rate limited, retry after {ms}ms"
    err_msg
        .strip_prefix("rate limited, retry after ")
        .and_then(|s| s.strip_suffix("ms"))
        .and_then(|s| s.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_retry_after_ms_from_error() {
        assert_eq!(
            parse_retry_after_ms("rate limited, retry after 5000ms"),
            Some(5000)
        );
        assert_eq!(parse_retry_after_ms("something else"), None);
    }
}
