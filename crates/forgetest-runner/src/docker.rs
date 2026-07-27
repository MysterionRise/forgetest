//! Docker-backed code runner.
//!
//! This runner keeps the local runner as the default path while providing an
//! opt-in, resource-limited execution boundary for portfolio and CI use.

use std::path::Path;
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use forgetest_core::model::Language;
use forgetest_core::results::{ClippyResult, CompilationResult, TestResult};
use forgetest_core::traits::{ClippyRequest, CodeRunner, CompileRequest, Dependency, TestRequest};

use crate::cargo_project::CargoProject;
use crate::repository_grader::{configure_process_group, force_remove_container, run_bounded};

/// Docker runner configuration.
#[derive(Debug, Clone)]
pub struct DockerRunnerConfig {
    pub image: String,
    pub memory: String,
    pub cpus: f64,
    pub pids_limit: u32,
    pub network: String,
    pub max_output_bytes: usize,
    pub tmpfs_size: String,
}

impl Default for DockerRunnerConfig {
    fn default() -> Self {
        Self {
            image: "forgetest-runner-rust:0.1.0".to_string(),
            memory: "512m".to_string(),
            cpus: 1.0,
            pids_limit: 128,
            network: "none".to_string(),
            max_output_bytes: 4 * 1024 * 1024,
            tmpfs_size: "256m".to_string(),
        }
    }
}

/// Code runner that executes Cargo inside a Docker container.
pub struct DockerRunner {
    shared_target_dir: std::path::PathBuf,
    default_timeout: Duration,
    default_dependencies: Vec<Dependency>,
    config: DockerRunnerConfig,
}

impl DockerRunner {
    pub fn new(shared_target_dir: std::path::PathBuf, config: DockerRunnerConfig) -> Self {
        Self {
            shared_target_dir,
            default_timeout: Duration::from_secs(120),
            default_dependencies: Vec::new(),
            config,
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

    fn create_project(&self, language: Language, timeout_secs: u64) -> Result<CargoProject> {
        let timeout = if timeout_secs > 0 {
            Duration::from_secs(timeout_secs)
        } else {
            self.default_timeout
        };
        let temp_parent = self
            .shared_target_dir
            .parent()
            .unwrap_or_else(|| Path::new("."));
        CargoProject::new_in(language, timeout, &self.shared_target_dir, temp_parent)
    }

    fn prepare_sandbox(
        &self,
        language: Language,
        code: &str,
        dependencies: &[Dependency],
        timeout_secs: u64,
    ) -> Result<CargoProject> {
        let sandbox = self.create_project(language, timeout_secs)?;
        sandbox.write_source(code)?;
        for dep in self.default_dependencies.iter().chain(dependencies.iter()) {
            ensure_docker_dependency_allowed(dep)?;
            sandbox.add_dependency(dep)?;
        }
        Ok(sandbox)
    }

    /// Build the `docker run` arguments for a Cargo command.
    pub fn docker_args_for(&self, sandbox: &CargoProject, cargo_args: &[&str]) -> Vec<String> {
        self.docker_args_for_named("forgetest-preview", sandbox, cargo_args)
    }

    fn docker_args_for_named(
        &self,
        container_name: &str,
        sandbox: &CargoProject,
        cargo_args: &[&str],
    ) -> Vec<String> {
        let user = non_root_user_spec(sandbox.work_dir());
        let work_mount = format!(
            "type=bind,src={},dst=/work",
            path_for_docker(sandbox.work_dir())
        );

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            container_name.to_string(),
            "--network".to_string(),
            self.config.network.clone(),
            "--memory".to_string(),
            self.config.memory.clone(),
            "--cpus".to_string(),
            format!("{:.2}", self.config.cpus),
            "--pids-limit".to_string(),
            self.config.pids_limit.to_string(),
            "--read-only".to_string(),
            "--cap-drop".to_string(),
            "ALL".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            "--user".to_string(),
            user,
            "--mount".to_string(),
            work_mount,
            "--tmpfs".to_string(),
            "/tmp:rw,noexec,nosuid,size=64m".to_string(),
            "--tmpfs".to_string(),
            format!(
                "/work/target:rw,exec,nosuid,size={}",
                self.config.tmpfs_size
            ),
            "--workdir".to_string(),
            "/work".to_string(),
            "--env".to_string(),
            "HOME=/tmp/home".to_string(),
            "--env".to_string(),
            "CARGO_TARGET_DIR=/work/target".to_string(),
            self.config.image.clone(),
            "cargo".to_string(),
        ];

        args.extend(cargo_args.iter().map(|arg| arg.to_string()));
        args
    }

    async fn run_docker(&self, sandbox: &CargoProject, cargo_args: &[&str]) -> Result<Output> {
        let container_name = format!("forgetest-snippet-{}", uuid::Uuid::new_v4().simple());
        let args = self.docker_args_for_named(&container_name, sandbox, cargo_args);
        let mut cmd = Command::new("docker");
        cmd.args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        if let Some(host) = std::env::var_os("DOCKER_HOST") {
            cmd.env("DOCKER_HOST", host);
        }
        configure_process_group(&mut cmd);
        let bounded = match run_bounded(cmd, sandbox.timeout(), self.config.max_output_bytes).await
        {
            Ok(output) => output,
            Err(error) => {
                force_remove_container(&container_name).await;
                return Err(error).context("Docker snippet runner failed");
            }
        };
        let output = Output {
            status: bounded.status,
            stdout: bounded.stdout,
            stderr: bounded.stderr,
        };
        ensure_no_docker_runtime_error(&output)?;
        Ok(output)
    }
}

#[async_trait]
impl CodeRunner for DockerRunner {
    async fn compile(&self, request: &CompileRequest) -> Result<CompilationResult> {
        let sandbox = self.prepare_sandbox(
            request.language,
            &request.code,
            &request.dependencies,
            request.timeout_secs,
        )?;
        let start = Instant::now();
        let output = self
            .run_docker(&sandbox, &["build", "--offline", "--message-format=json"])
            .await?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (errors, warnings) = crate::compiler::parse_cargo_json_output(&stdout);

        Ok(CompilationResult {
            success: output.status.success(),
            errors,
            warnings,
            duration_ms,
        })
    }

    async fn run_tests(&self, request: &TestRequest) -> Result<TestResult> {
        let sandbox = self.prepare_sandbox(
            request.language,
            &request.code,
            &request.dependencies,
            request.timeout_secs,
        )?;
        sandbox.write_test(&request.test_code)?;

        let start = Instant::now();
        let output = self.run_docker(&sandbox, &["test", "--offline"]).await?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        crate::test_runner::parse_test_command_output(
            &combined,
            duration_ms,
            output.status.success(),
        )
    }

    async fn run_clippy(&self, request: &ClippyRequest) -> Result<ClippyResult> {
        let sandbox = self.prepare_sandbox(
            request.language,
            &request.code,
            &request.dependencies,
            request.timeout_secs,
        )?;
        let output = self
            .run_docker(
                &sandbox,
                &[
                    "clippy",
                    "--offline",
                    "--message-format=json",
                    "--",
                    "-W",
                    "clippy::all",
                ],
            )
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let warnings = crate::clippy::parse_clippy_output(&stdout);

        Ok(ClippyResult {
            warning_count: warnings.len() as u32,
            warnings,
        })
    }
}

/// Validate that a dependency is available in the bundled offline Docker image.
pub fn ensure_docker_dependency_allowed(dep: &Dependency) -> Result<()> {
    let tokio_features = ["full"];
    let allowed = dep.name == "tokio"
        && dep.version == "1"
        && dep
            .features
            .iter()
            .all(|f| tokio_features.contains(&f.as_str()));

    anyhow::ensure!(
        allowed,
        "Docker runner only supports bundled allowlisted dependencies in v0.1; unsupported dependency '{} {}'",
        dep.name,
        dep.version
    );
    Ok(())
}

fn path_for_docker(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn ensure_no_docker_runtime_error(output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let is_docker_error = matches!(output.status.code(), Some(125..=127))
        || stderr.contains("Docker daemon")
        || stderr.contains("docker daemon")
        || stderr.contains("Error response from daemon")
        || stderr.contains("pull access denied")
        || stderr.contains("No such image")
        || stderr.contains("permission denied while trying to connect");

    anyhow::ensure!(
        !is_docker_error,
        "Docker runner failed before Cargo executed: {}",
        stderr.trim()
    );

    Ok(())
}

#[cfg(unix)]
fn non_root_user_spec(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;

    let Ok(meta) = std::fs::metadata(path) else {
        return "1000:1000".to_string();
    };
    let uid = meta.uid();
    let gid = meta.gid();
    if uid == 0 {
        "1000:1000".to_string()
    } else {
        format!("{uid}:{gid}")
    }
}

#[cfg(not(unix))]
fn non_root_user_spec(_path: &Path) -> String {
    "1000:1000".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgetest_core::model::Language;

    #[test]
    fn runner_image_definition_is_pinned_and_non_root() {
        let dockerfile = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docker/forgetest-runner-rust.Dockerfile"),
        )
        .unwrap();
        let cache_manifest = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/runner-cache/Cargo.toml"),
        )
        .unwrap();

        assert_eq!(dockerfile.lines().next(), Some("FROM rust:1.92.0-bookworm"));
        assert!(dockerfile.contains("cargo fetch --locked"));
        assert!(dockerfile.contains("USER 10001:10001"));
        assert!(dockerfile.contains("CARGO_NET_OFFLINE=true"));
        assert!(cache_manifest.contains("version = \"=1.49.0\""));
    }

    #[test]
    fn docker_args_include_security_limits() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();
        let runner = DockerRunner::new(target.path().to_path_buf(), DockerRunnerConfig::default());

        let args = runner.docker_args_for(&sandbox, &["build", "--offline"]);

        assert!(args.windows(2).any(|w| w == ["--network", "none"]));
        assert!(args.windows(2).any(|w| w == ["--memory", "512m"]));
        assert!(args.windows(2).any(|w| w == ["--pids-limit", "128"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--name", "forgetest-preview"]));
        assert!(args.iter().any(|arg| arg == "--user"));
        assert!(args.iter().any(|arg| arg == "--read-only"));
        assert!(args.windows(2).any(|w| w == ["--cap-drop", "ALL"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--security-opt", "no-new-privileges"]));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--tmpfs" && w[1].starts_with("/tmp:")));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--tmpfs" && w[1].starts_with("/work/target:")));
        assert!(args
            .windows(2)
            .filter(|w| w[0] == "--tmpfs" && w[1].starts_with("/work/target:"))
            .all(|w| {
                let options: Vec<_> = w[1].split(',').collect();
                options.contains(&"exec") && !options.contains(&"noexec")
            }));
        assert_eq!(
            args.iter().filter(|arg| arg.as_str() == "--mount").count(),
            1
        );
        assert!(args
            .windows(2)
            .filter(|pair| pair[0] == "--mount")
            .all(|pair| !pair[1].ends_with(",rw")));
        assert!(!args
            .iter()
            .any(|arg| arg.contains(sandbox.shared_target_dir().to_string_lossy().as_ref())));
        assert!(args.iter().any(|arg| arg == "forgetest-runner-rust:0.1.0"));
        assert!(args.ends_with(&[
            "cargo".to_string(),
            "build".to_string(),
            "--offline".to_string()
        ]));
    }

    #[test]
    fn docker_dependency_allowlist_accepts_tokio_full() {
        let dep = Dependency {
            name: "tokio".into(),
            version: "1".into(),
            features: vec!["full".into()],
        };
        assert!(ensure_docker_dependency_allowed(&dep).is_ok());
    }

    #[test]
    fn docker_dependency_allowlist_rejects_arbitrary_crate() {
        let dep = Dependency {
            name: "reqwest".into(),
            version: "0.12".into(),
            features: vec![],
        };
        let err = ensure_docker_dependency_allowed(&dep).unwrap_err();
        assert!(err.to_string().contains("allowlisted"));
    }

    #[cfg(unix)]
    #[test]
    fn docker_runtime_error_is_not_reported_as_compile_failure() {
        use std::os::unix::process::ExitStatusExt;

        let output = Output {
            status: std::process::ExitStatus::from_raw(126 << 8),
            stdout: Vec::new(),
            stderr:
                b"docker: Got permission denied while trying to connect to the Docker daemon socket"
                    .to_vec(),
        };
        let err = ensure_no_docker_runtime_error(&output).unwrap_err();
        assert!(err
            .to_string()
            .contains("Docker runner failed before Cargo executed"));
    }

    #[tokio::test]
    async fn docker_executes_compile_when_enabled() {
        if std::env::var("FORGETEST_DOCKER_TEST").ok().as_deref() != Some("1") {
            return;
        }

        let target = tempfile::tempdir().unwrap();
        let runner = DockerRunner::new(target.path().to_path_buf(), DockerRunnerConfig::default());
        let result = runner
            .compile(&CompileRequest {
                code: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
                language: Language::Rust,
                dependencies: vec![],
                timeout_secs: 120,
            })
            .await
            .unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn docker_executes_and_counts_tests_when_enabled() {
        if std::env::var("FORGETEST_DOCKER_TEST").ok().as_deref() != Some("1") {
            return;
        }

        let target = tempfile::tempdir().unwrap();
        let runner = DockerRunner::new(target.path().to_path_buf(), DockerRunnerConfig::default());
        let sandbox = runner
            .prepare_sandbox(
                Language::Rust,
                "pub fn add(a: i32, b: i32) -> i32 { a + b }",
                &[],
                120,
            )
            .unwrap();
        sandbox
            .write_test(
                "#[cfg(test)] mod tests { use super::*; #[test] fn adds() { assert_eq!(add(2, 3), 5); } }",
            )
            .unwrap();
        let output = runner
            .run_docker(&sandbox, &["test", "--offline"])
            .await
            .unwrap();
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let parsed = crate::test_runner::parse_test_output(&combined, 0).unwrap();

        assert_eq!(
            parsed.passed, 1,
            "Docker test output was not counted:\n{combined}"
        );
        assert_eq!(parsed.failed, 0);
    }
}
