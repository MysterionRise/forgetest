//! Central eval engine orchestrator.
//!
//! Coordinates multiple eval cases across multiple models with parallelism,
//! retries, and Pass@k support.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::error::ProviderError;
use crate::model::EvalSet;
use crate::report::{EvalReport, EvalSetSummary, RunManifest};
use crate::results::{
    CompilationResult, CompilerDiagnostic, DiagnosticLevel, EvalResult, EvalResultStatus, Score,
    TimingInfo, TokenUsage,
};
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
    /// Optional provenance manifest to attach to the report.
    pub manifest: Option<RunManifest>,
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
            manifest: None,
        }
    }
}

/// Which model to evaluate.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Provider name (e.g. "anthropic").
    pub provider: String,
    /// Exact provider model identifier.
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
        let default_language = eval_set.default_language;
        let default_timeout_secs = eval_set.default_timeout_secs;

        let mut futures = FuturesUnordered::new();

        for model_spec in models {
            let provider = self.providers.get(&model_spec.provider).ok_or_else(|| {
                anyhow::anyhow!(
                    "provider '{}' is not configured for model '{}'",
                    model_spec.provider,
                    model_spec.model
                )
            })?;

            for case in &eval_set.cases {
                for attempt in 1..=max_k {
                    progress.on_eval_start(&case.id, &model_spec.model, attempt);
                    let provider = Arc::clone(provider);
                    let runner = Arc::clone(&self.runner);
                    let semaphore = Arc::clone(&semaphore);
                    let case = case.clone();
                    let model = model_spec.model.clone();
                    let provider_name = model_spec.provider.clone();
                    let config = self.config.clone();

                    futures.push(async move {
                        let task_start = Instant::now();
                        let ctx_case_id = case.id.clone();
                        let ctx_model = model.clone();
                        let ctx_provider = provider_name.clone();
                        let inner = async move {
                            let _permit = semaphore
                                .clone()
                                .acquire_owned()
                                .await
                                .map_err(|_| anyhow::anyhow!("semaphore closed"))?;
                            let execution_start = Instant::now();

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
                                        let language = case.language.unwrap_or(default_language);
                                        let timeout_secs =
                                            case.timeout_secs.unwrap_or(default_timeout_secs);

                                        let deps = case.dependencies.clone();

                                        // Compile the generated code
                                        let compile_result = runner
                                            .compile(&CompileRequest {
                                                code: generated_code.clone(),
                                                language,
                                                dependencies: deps.clone(),
                                                timeout_secs,
                                            })
                                            .await
                                            .context("runner compile failed")?;
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
                                                            dependencies: deps.clone(),
                                                            timeout_secs,
                                                        })
                                                        .await
                                                        .context("runner test execution failed")?,
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
                                                        dependencies: deps,
                                                        timeout_secs,
                                                    })
                                                    .await
                                                    .context("runner clippy execution failed")?,
                                            )
                                        } else {
                                            None
                                        };

                                        let total_ms = execution_start.elapsed().as_millis() as u64;

                                        let mut eval_result = EvalResult {
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
                                            score: None,
                                            status: EvalResultStatus::Completed,
                                            error: None,
                                            attempt,
                                            run_id,
                                        };
                                        eval_result.score =
                                            Some(Score::compute(&eval_result, &case.expectations));

                                        return Ok(eval_result);
                                    }
                                    Err(e) => {
                                        // Downcast to ProviderError for proper classification
                                        if let Some(provider_err) =
                                            e.downcast_ref::<ProviderError>()
                                        {
                                            if provider_err.is_permanent() {
                                                return Err(e.context("provider generation failed"));
                                            }
                                            if let Some(ms) = provider_err.retry_after_ms() {
                                                retry_delay = Duration::from_millis(ms);
                                            }
                                        }
                                        last_error = Some(e);
                                    }
                                }
                            }

                            Err(last_error
                                .unwrap_or_else(|| anyhow::anyhow!("unknown provider error"))
                                .context("provider generation failed"))
                        };
                        let result = inner.await;
                        (
                            ctx_case_id,
                            ctx_model,
                            ctx_provider,
                            attempt,
                            task_start.elapsed(),
                            result,
                        )
                    });
                }
            }
        }

        let mut results = Vec::new();
        let mut completed = 0usize;
        let mut failed = 0usize;
        let total = futures.len();

        while let Some((case_id, model, provider, attempt, task_elapsed, result)) =
            futures.next().await
        {
            match result {
                Ok(eval_result) => {
                    progress.on_eval_complete(&eval_result);
                    results.push(eval_result);
                    completed += 1;
                }
                Err(e) => {
                    let error = format!("{e:#}");
                    tracing::error!("eval failed for {case_id}/{model}: {error}");
                    progress.on_eval_error(&case_id, &model, &error);
                    let status = if error.contains("provider generation failed") {
                        EvalResultStatus::ProviderError
                    } else {
                        EvalResultStatus::RunnerError
                    };
                    results.push(failed_eval_result(
                        &case_id,
                        &model,
                        &provider,
                        attempt,
                        run_id,
                        status,
                        error,
                        task_elapsed,
                    ));
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
            manifest: self.config.manifest.clone(),
            duration_ms: elapsed.as_millis() as u64,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn failed_eval_result(
    case_id: &str,
    model: &str,
    provider: &str,
    attempt: u32,
    run_id: Uuid,
    status: EvalResultStatus,
    error: String,
    elapsed: Duration,
) -> EvalResult {
    EvalResult {
        case_id: case_id.to_string(),
        model: model.to_string(),
        provider: provider.to_string(),
        generated_code: String::new(),
        compilation: CompilationResult {
            success: false,
            errors: vec![CompilerDiagnostic {
                level: DiagnosticLevel::Error,
                message: error.clone(),
                code: None,
                spans: Vec::new(),
            }],
            warnings: Vec::new(),
            duration_ms: 0,
        },
        test_execution: None,
        clippy: None,
        timing: TimingInfo {
            llm_request_ms: elapsed.as_millis() as u64,
            compilation_ms: 0,
            test_execution_ms: 0,
            total_ms: elapsed.as_millis() as u64,
        },
        token_usage: TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
        },
        score: Some(Score {
            compilation: 0.0,
            tests: 0.0,
            clippy: 0.0,
            structure: 0.0,
            overall: 0.0,
        }),
        status,
        error: Some(error),
        attempt,
        run_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EvalCase, Expectations, Language};
    use crate::results::{ClippyResult, CompilationResult, TestResult, TokenUsage};
    use crate::traits::{ClippyRequest, CompileRequest, GenerateResponse, ModelInfo, TestRequest};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct FixedProvider {
        error: bool,
    }

    #[async_trait]
    impl LlmProvider for FixedProvider {
        fn name(&self) -> &str {
            "test"
        }

        async fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse> {
            if self.error {
                anyhow::bail!("provider unavailable");
            }
            Ok(GenerateResponse {
                content: "pub fn answer() -> u32 { 42 }".into(),
                extracted_code: "pub fn answer() -> u32 { 42 }".into(),
                model: request.model.clone(),
                token_usage: TokenUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    estimated_cost_usd: 0.0,
                },
                latency_ms: 1,
            })
        }

        fn available_models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }
    }

    #[derive(Default)]
    struct RecordingRunner {
        compile_requests: Mutex<Vec<CompileRequest>>,
    }

    #[async_trait]
    impl CodeRunner for RecordingRunner {
        async fn compile(&self, request: &CompileRequest) -> Result<CompilationResult> {
            self.compile_requests.lock().unwrap().push(request.clone());
            Ok(CompilationResult {
                success: false,
                errors: Vec::new(),
                warnings: Vec::new(),
                duration_ms: 1,
            })
        }

        async fn run_tests(&self, _: &TestRequest) -> Result<TestResult> {
            panic!("tests should not run after compilation failure")
        }

        async fn run_clippy(&self, _: &ClippyRequest) -> Result<ClippyResult> {
            panic!("clippy should not run after compilation failure")
        }
    }

    struct SlowClippyRunner;

    struct SlowFailProvider;

    #[async_trait]
    impl LlmProvider for SlowFailProvider {
        fn name(&self) -> &str {
            "test"
        }

        async fn generate(&self, _: &GenerateRequest) -> Result<GenerateResponse> {
            tokio::time::sleep(Duration::from_millis(30)).await;
            anyhow::bail!("provider unavailable")
        }

        fn available_models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }
    }

    #[async_trait]
    impl CodeRunner for SlowClippyRunner {
        async fn compile(&self, _: &CompileRequest) -> Result<CompilationResult> {
            Ok(CompilationResult {
                success: true,
                errors: Vec::new(),
                warnings: Vec::new(),
                duration_ms: 1,
            })
        }

        async fn run_tests(&self, _: &TestRequest) -> Result<TestResult> {
            panic!("test execution is disabled for this fixture")
        }

        async fn run_clippy(&self, _: &ClippyRequest) -> Result<ClippyResult> {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok(ClippyResult {
                warnings: Vec::new(),
                warning_count: 0,
            })
        }
    }

    fn eval_set(timeout: u64) -> EvalSet {
        EvalSet {
            id: "set".into(),
            name: "Set".into(),
            description: String::new(),
            cases: vec![EvalCase {
                id: "case".into(),
                name: "Case".into(),
                description: String::new(),
                prompt: "Implement answer".into(),
                language: None,
                context: Vec::new(),
                expectations: Expectations {
                    should_pass_tests: false,
                    ..Expectations::default()
                },
                tags: Vec::new(),
                dependencies: Vec::new(),
                timeout_secs: None,
                max_tokens: None,
            }],
            default_language: Language::Rust,
            default_timeout_secs: timeout,
        }
    }

    fn model_specs() -> Vec<ModelSpec> {
        vec![ModelSpec {
            provider: "test".into(),
            model: "fixed".into(),
        }]
    }

    #[test]
    fn provider_error_classification() {
        let rate_limited = ProviderError::RateLimited {
            retry_after_ms: 5000,
        };
        assert!(!rate_limited.is_permanent());
        assert_eq!(rate_limited.retry_after_ms(), Some(5000));

        let auth_failed = ProviderError::AuthenticationFailed("bad key".into());
        assert!(auth_failed.is_permanent());
        assert_eq!(auth_failed.retry_after_ms(), None);

        let not_found = ProviderError::ModelNotFound("gpt-99".into());
        assert!(not_found.is_permanent());

        let timeout = ProviderError::Timeout(120);
        assert!(!timeout.is_permanent());
    }

    #[tokio::test]
    async fn scheduled_provider_failure_is_recorded_in_results() {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        providers.insert("test".into(), Arc::new(FixedProvider { error: true }));
        let runner = Arc::new(RecordingRunner::default());
        let engine = EvalEngine::new(
            providers,
            runner,
            EvalEngineConfig {
                max_retries_per_case: 0,
                ..EvalEngineConfig::default()
            },
        );

        let report = engine
            .run(&eval_set(17), &model_specs(), &NoopReporter)
            .await
            .unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].score.as_ref().unwrap().overall, 0.0);
    }

    #[tokio::test]
    async fn case_inherits_eval_set_timeout() {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        providers.insert("test".into(), Arc::new(FixedProvider { error: false }));
        let runner = Arc::new(RecordingRunner::default());
        let engine = EvalEngine::new(
            providers,
            Arc::clone(&runner) as Arc<dyn CodeRunner>,
            EvalEngineConfig::default(),
        );

        engine
            .run(&eval_set(17), &model_specs(), &NoopReporter)
            .await
            .unwrap();

        let requests = runner.compile_requests.lock().unwrap();
        assert_eq!(requests[0].timeout_secs, 17);
        assert_eq!(requests[0].language, Language::Rust);
    }

    #[tokio::test]
    async fn trial_total_includes_clippy_execution() {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        providers.insert("test".into(), Arc::new(FixedProvider { error: false }));
        let engine = EvalEngine::new(
            providers,
            Arc::new(SlowClippyRunner),
            EvalEngineConfig::default(),
        );

        let report = engine
            .run(&eval_set(17), &model_specs(), &NoopReporter)
            .await
            .unwrap();

        assert!(report.results[0].timing.total_ms >= 25);
    }

    #[tokio::test]
    async fn failed_trial_total_includes_provider_execution() {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        providers.insert("test".into(), Arc::new(SlowFailProvider));
        let engine = EvalEngine::new(
            providers,
            Arc::new(RecordingRunner::default()),
            EvalEngineConfig {
                max_retries_per_case: 0,
                ..EvalEngineConfig::default()
            },
        );

        let report = engine
            .run(&eval_set(17), &model_specs(), &NoopReporter)
            .await
            .unwrap();

        assert_eq!(report.results[0].status, EvalResultStatus::ProviderError);
        assert!(report.results[0].timing.total_ms >= 25);
    }
}
