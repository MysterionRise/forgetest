//! forgetest-runner — Sandboxed compilation and test execution.
//!
//! Creates isolated Cargo projects for each eval, compiles generated code,
//! runs tests, and collects clippy diagnostics.

pub mod clippy;
pub mod compiler;
pub mod sandbox;
pub mod test_runner;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use forgetest_core::model::{EvalCase, Language};
use forgetest_core::results::{
    ClippyResult, CompilationResult, EvalResult, TestResult, TimingInfo, TokenUsage,
};
use forgetest_core::traits::{ClippyRequest, CodeRunner, CompileRequest, Dependency, TestRequest};

/// Local code runner that uses sandboxed Cargo projects.
pub struct LocalRunner {
    /// Shared target directory for caching compiled dependencies.
    shared_target_dir: PathBuf,
    /// Default timeout for compilation and tests.
    default_timeout: Duration,
    /// Default dependencies added to every sandbox.
    default_dependencies: Vec<Dependency>,
}

impl LocalRunner {
    pub fn new(shared_target_dir: PathBuf) -> Self {
        Self {
            shared_target_dir,
            default_timeout: Duration::from_secs(120),
            default_dependencies: Vec::new(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<Dependency>) -> Self {
        self.default_dependencies = deps;
        self
    }

    fn create_sandbox(&self, language: Language, timeout_secs: u64) -> Result<sandbox::Sandbox> {
        let timeout = if timeout_secs > 0 {
            Duration::from_secs(timeout_secs)
        } else {
            self.default_timeout
        };
        sandbox::Sandbox::new(language, timeout, &self.shared_target_dir)
    }
}

#[async_trait]
impl CodeRunner for LocalRunner {
    async fn compile(&self, request: &CompileRequest) -> Result<CompilationResult> {
        let sandbox = self.create_sandbox(request.language, request.timeout_secs)?;
        sandbox.write_source(&request.code)?;
        for dep in self
            .default_dependencies
            .iter()
            .chain(request.dependencies.iter())
        {
            sandbox.add_dependency(dep)?;
        }
        compiler::compile(&sandbox).await
    }

    async fn run_tests(&self, request: &TestRequest) -> Result<TestResult> {
        let sandbox = self.create_sandbox(request.language, request.timeout_secs)?;
        sandbox.write_source(&request.code)?;
        sandbox.write_test(&request.test_code)?;
        for dep in self
            .default_dependencies
            .iter()
            .chain(request.dependencies.iter())
        {
            sandbox.add_dependency(dep)?;
        }

        test_runner::run_tests(&sandbox).await
    }

    async fn run_clippy(&self, request: &ClippyRequest) -> Result<ClippyResult> {
        let sandbox = self.create_sandbox(request.language, request.timeout_secs)?;
        sandbox.write_source(&request.code)?;
        for dep in self
            .default_dependencies
            .iter()
            .chain(request.dependencies.iter())
        {
            sandbox.add_dependency(dep)?;
        }
        clippy::run_clippy(&sandbox).await
    }
}

/// Run a full eval: compile, test, clippy, compute score.
#[allow(clippy::too_many_arguments)]
pub async fn run_eval(
    runner: &LocalRunner,
    case: &EvalCase,
    generated_code: &str,
    model: &str,
    provider: &str,
    token_usage: TokenUsage,
    llm_request_ms: u64,
    attempt: u32,
    run_id: Uuid,
) -> Result<EvalResult> {
    let language = case.language.unwrap_or(Language::Rust);
    let timeout_secs = case.timeout_secs.unwrap_or(60);
    let sandbox = runner.create_sandbox(language, timeout_secs)?;

    sandbox.write_source(generated_code)?;

    // Compile
    let compilation = compiler::compile(&sandbox).await?;
    let compilation_ms = compilation.duration_ms;

    // Run tests if compilation succeeded and tests are expected
    let test_execution = if compilation.success && case.expectations.should_pass_tests {
        if let Some(test_file) = &case.expectations.test_file {
            sandbox.write_test(test_file)?;
            // Need to recompile with tests
            let _recompile = compiler::compile(&sandbox).await?;
            Some(test_runner::run_tests(&sandbox).await?)
        } else {
            None
        }
    } else {
        None
    };
    let test_execution_ms = test_execution.as_ref().map(|t| t.duration_ms).unwrap_or(0);

    // Run clippy if compilation succeeded
    let clippy_result = if compilation.success {
        Some(clippy::run_clippy(&sandbox).await?)
    } else {
        None
    };

    let total_ms = llm_request_ms + compilation_ms + test_execution_ms;

    Ok(EvalResult {
        case_id: case.id.clone(),
        model: model.to_string(),
        provider: provider.to_string(),
        generated_code: generated_code.to_string(),
        compilation,
        test_execution,
        clippy: clippy_result,
        timing: TimingInfo {
            llm_request_ms,
            compilation_ms,
            test_execution_ms,
            total_ms,
        },
        token_usage,
        attempt,
        run_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compile_valid_code() {
        let target = tempfile::tempdir().unwrap();
        let runner = LocalRunner::new(target.path().to_path_buf());

        let request = CompileRequest {
            code: "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            language: Language::Rust,
            dependencies: vec![],
            timeout_secs: 120,
        };

        let result = runner.compile(&request).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn run_tests_passing() {
        let target = tempfile::tempdir().unwrap();
        let runner = LocalRunner::new(target.path().to_path_buf());

        let request = TestRequest {
            code: "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            test_code: r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
        assert_eq!(add(0, 0), 0);
    }
}
"#
            .to_string(),
            language: Language::Rust,
            dependencies: vec![],
            timeout_secs: 120,
        };

        let result = runner.run_tests(&request).await.unwrap();
        assert!(
            result.passed >= 1,
            "expected at least 1 passing test, got {}",
            result.passed
        );
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn run_tests_failing() {
        let target = tempfile::tempdir().unwrap();
        let runner = LocalRunner::new(target.path().to_path_buf());

        let request = TestRequest {
            code: "pub fn add(a: i32, b: i32) -> i32 { a - b }".to_string(), // Bug: subtracts instead
            test_code: r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}
"#
            .to_string(),
            language: Language::Rust,
            dependencies: vec![],
            timeout_secs: 120,
        };

        let result = runner.run_tests(&request).await.unwrap();
        assert_eq!(result.failed, 1);
    }

    #[tokio::test]
    async fn full_eval_pipeline() {
        let target = tempfile::tempdir().unwrap();
        let runner = LocalRunner::new(target.path().to_path_buf());

        let case = EvalCase {
            id: "test-add".into(),
            name: "Add function".into(),
            description: "Test add".into(),
            prompt: "Write add".into(),
            language: Some(Language::Rust),
            context: vec![],
            expectations: forgetest_core::model::Expectations {
                should_compile: true,
                should_pass_tests: true,
                test_file: Some(
                    r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() { assert_eq!(add(1, 2), 3); }
}
"#
                    .to_string(),
                ),
                expected_functions: vec!["add".into()],
                ..Default::default()
            },
            tags: vec![],
            timeout_secs: Some(120),
            max_tokens: None,
        };

        let code = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
        let token_usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            estimated_cost_usd: 0.0,
        };

        let result = run_eval(
            &runner,
            &case,
            code,
            "mock",
            "mock",
            token_usage,
            100,
            1,
            Uuid::nil(),
        )
        .await
        .unwrap();

        assert!(result.compilation.success);
        assert!(result.test_execution.is_some());
        let tests = result.test_execution.as_ref().unwrap();
        assert!(tests.passed >= 1);
        assert_eq!(tests.failed, 0);

        let score = forgetest_core::results::Score::compute(&result, &case.expectations);
        assert!(
            score.overall > 0.5,
            "overall score should be > 0.5, got {}",
            score.overall
        );
    }
}
