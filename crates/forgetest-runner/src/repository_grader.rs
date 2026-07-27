//! Independent local and hardened Docker repository graders.

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use forgetest_core::agent::{GradeCheckRequest, GradeRequest, Grader};
use forgetest_core::repository_report::{GraderCheck, GraderOutcome};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

/// Trusted host grader used for local development.
pub struct LocalRepositoryGrader {
    max_output_bytes: usize,
}

impl LocalRepositoryGrader {
    pub fn new(max_output_bytes: usize) -> Self {
        Self { max_output_bytes }
    }
}

#[async_trait]
impl Grader for LocalRepositoryGrader {
    fn identity(&self) -> String {
        "local-rust-verifier-v1".into()
    }

    async fn grade(&self, request: &GradeRequest) -> Result<GraderOutcome> {
        validate_grade_request(request, self.max_output_bytes)?;
        let isolated_home =
            tempfile::tempdir().context("failed to create isolated verifier home")?;
        let start = Instant::now();
        let mut outputs = Vec::with_capacity(request.checks.len());
        let mut used_output = 0;
        for (index, check) in request.checks.iter().enumerate() {
            let remaining_time = request
                .timeout
                .checked_sub(start.elapsed())
                .context("verifier timed out before all checks completed")?;
            let remaining_output = self
                .max_output_bytes
                .checked_sub(used_output)
                .context("verifier exceeded output limit")?;
            let mut command = Command::new(&check.command[0]);
            command
                .args(&check.command[1..])
                .current_dir(&request.workspace)
                .env_clear()
                .env("HOME", isolated_home.path())
                .env("USERPROFILE", isolated_home.path())
                .env("TMPDIR", isolated_home.path())
                .env("TMP", isolated_home.path())
                .env("TEMP", isolated_home.path())
                .env(
                    "CARGO_TARGET_DIR",
                    request.workspace.join(format!("target-check-{index}")),
                )
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            copy_toolchain_environment(&mut command);
            configure_process_group(&mut command);
            let output = run_bounded(command, remaining_time, remaining_output).await?;
            used_output += output.stdout.len() + output.stderr.len();
            outputs.push((check, output));
        }
        Ok(grader_outcome(outputs, start.elapsed().as_millis() as u64))
    }
}

/// Security policy for the Docker verifier.
#[derive(Debug, Clone)]
pub struct DockerVerifierConfig {
    pub image: String,
    pub memory: String,
    pub cpus: f64,
    pub pids_limit: u32,
    pub max_output_bytes: usize,
    pub tmpfs_size: String,
}

impl Default for DockerVerifierConfig {
    fn default() -> Self {
        Self {
            image: "forgetest-runner-rust:0.1.0".into(),
            memory: "1g".into(),
            cpus: 1.0,
            pids_limit: 128,
            max_output_bytes: 4 * 1024 * 1024,
            tmpfs_size: "256m".into(),
        }
    }
}

/// Network-disabled, resource-limited Docker verifier.
pub struct DockerRepositoryGrader {
    config: DockerVerifierConfig,
}

impl DockerRepositoryGrader {
    pub fn new(config: DockerVerifierConfig) -> Self {
        Self { config }
    }

    pub fn docker_args(
        &self,
        container_name: &str,
        workspace: &Path,
        command: &[String],
    ) -> Vec<String> {
        let workspace_mount = format!("type=bind,src={},dst=/work", workspace.to_string_lossy());
        let mut arguments = vec![
            "run".into(),
            "--rm".into(),
            "--name".into(),
            container_name.into(),
            "--network".into(),
            "none".into(),
            "--read-only".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
            "--memory".into(),
            self.config.memory.clone(),
            "--cpus".into(),
            format!("{:.2}", self.config.cpus),
            "--pids-limit".into(),
            self.config.pids_limit.to_string(),
            "--user".into(),
            non_root_user_spec(workspace),
            "--mount".into(),
            workspace_mount,
            "--tmpfs".into(),
            "/tmp:rw,noexec,nosuid,size=64m".into(),
            "--tmpfs".into(),
            format!(
                "/work/target:rw,exec,nosuid,size={}",
                self.config.tmpfs_size
            ),
            "--workdir".into(),
            "/work".into(),
            "--env".into(),
            "HOME=/tmp/home".into(),
            "--env".into(),
            "CARGO_TARGET_DIR=/work/target".into(),
            self.config.image.clone(),
        ];
        arguments.extend(command.iter().cloned());
        arguments
    }
}

#[async_trait]
impl Grader for DockerRepositoryGrader {
    fn identity(&self) -> String {
        format!("docker-rust-verifier-v1:{}", self.config.image)
    }

    async fn grade(&self, request: &GradeRequest) -> Result<GraderOutcome> {
        validate_grade_request(request, self.config.max_output_bytes)?;
        let start = Instant::now();
        let mut outputs = Vec::with_capacity(request.checks.len());
        let mut used_output = 0;
        for (index, check) in request.checks.iter().enumerate() {
            let container_name = format!("forgetest-{}-{index}", request.trial_id.simple());
            let mut command = Command::new("docker");
            command
                .args(self.docker_args(&container_name, &request.workspace, &check.command))
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            if let Some(path) = std::env::var_os("PATH") {
                command.env("PATH", path);
            }
            configure_process_group(&mut command);
            let remaining_time = request
                .timeout
                .checked_sub(start.elapsed())
                .context("verifier timed out before all checks completed")?;
            let remaining_output = self
                .config
                .max_output_bytes
                .checked_sub(used_output)
                .context("verifier exceeded output limit")?;
            let output = match run_bounded(command, remaining_time, remaining_output).await {
                Ok(output) => output,
                Err(error) => {
                    force_remove_container(&container_name).await;
                    return Err(error);
                }
            };
            ensure_docker_started(&output)?;
            used_output += output.stdout.len() + output.stderr.len();
            outputs.push((check, output));
        }
        Ok(grader_outcome(outputs, start.elapsed().as_millis() as u64))
    }
}

pub(crate) struct BoundedOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(crate) async fn run_bounded(
    mut command: Command,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> Result<BoundedOutput> {
    let mut child = command
        .spawn()
        .context("failed to start verifier command")?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .context("verifier stdout was not captured")?;
    let stderr = child
        .stderr
        .take()
        .context("verifier stderr was not captured")?;
    let used = Arc::new(AtomicUsize::new(0));
    let exceeded = Arc::new(AtomicBool::new(false));
    let notify = Arc::new(Notify::new());
    let stdout_task = tokio::spawn(collect_bounded(
        stdout,
        max_output_bytes,
        Arc::clone(&used),
        Arc::clone(&exceeded),
        Arc::clone(&notify),
    ));
    let stderr_task = tokio::spawn(collect_bounded(
        stderr,
        max_output_bytes,
        Arc::clone(&used),
        Arc::clone(&exceeded),
        Arc::clone(&notify),
    ));

    enum Stop {
        Exited(ExitStatus),
        Timeout,
        OutputLimit,
    }
    let stop = tokio::select! {
        status = child.wait() => Stop::Exited(status.context("failed waiting for verifier")?),
        _ = tokio::time::sleep(timeout) => Stop::Timeout,
        _ = notify.notified() => Stop::OutputLimit,
    };
    if !matches!(stop, Stop::Exited(_)) {
        terminate_process_tree(&mut child, pid).await;
    }
    let stdout = stdout_task.await.context("stdout collector failed")??;
    let stderr = stderr_task.await.context("stderr collector failed")??;
    let output_exceeded = exceeded.load(Ordering::SeqCst);
    match stop {
        Stop::Exited(_) if output_exceeded => {
            anyhow::bail!("verifier exceeded output limit of {max_output_bytes} bytes")
        }
        Stop::Exited(status) => Ok(BoundedOutput {
            status,
            stdout,
            stderr,
        }),
        Stop::Timeout => anyhow::bail!("verifier timed out after {}ms", timeout.as_millis()),
        Stop::OutputLimit => {
            anyhow::bail!("verifier exceeded output limit of {max_output_bytes} bytes")
        }
    }
}

async fn collect_bounded<R>(
    mut reader: R,
    limit: usize,
    used: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
    notify: Arc<Notify>,
) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut collected = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let previous = used.fetch_add(read, Ordering::Relaxed);
        if previous < limit {
            let keep = read.min(limit - previous);
            collected.extend_from_slice(&buffer[..keep]);
        }
        if previous.saturating_add(read) > limit && !exceeded.swap(true, Ordering::SeqCst) {
            notify.notify_one();
        }
    }
    Ok(collected)
}

fn validate_grade_request(request: &GradeRequest, max_output_bytes: usize) -> Result<()> {
    anyhow::ensure!(
        request.workspace.is_dir(),
        "verifier workspace is not a directory: {}",
        request.workspace.display()
    );
    anyhow::ensure!(!request.checks.is_empty(), "verifier checks are empty");
    for check in &request.checks {
        anyhow::ensure!(
            !check.name.trim().is_empty(),
            "verifier check name is empty"
        );
        anyhow::ensure!(
            !check.command.is_empty(),
            "verifier check '{}' command is empty",
            check.name
        );
    }
    anyhow::ensure!(
        !request.timeout.is_zero(),
        "verifier timeout must be positive"
    );
    anyhow::ensure!(
        max_output_bytes > 0,
        "verifier output limit must be positive"
    );
    Ok(())
}

fn grader_outcome(
    outputs: Vec<(&GradeCheckRequest, BoundedOutput)>,
    duration_ms: u64,
) -> GraderOutcome {
    let success = outputs.iter().all(|(_, output)| output.status.success());
    let exit_code = outputs
        .iter()
        .find(|(_, output)| !output.status.success())
        .and_then(|(_, output)| output.status.code())
        .or(Some(0));
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut checks = Vec::with_capacity(outputs.len());
    for (check, output) in outputs {
        let output_stdout = String::from_utf8_lossy(&output.stdout);
        let output_stderr = String::from_utf8_lossy(&output.stderr);
        stdout.push_str(&format!("== {} ==\n{output_stdout}", check.name));
        stderr.push_str(&format!("== {} ==\n{output_stderr}", check.name));
        checks.push(GraderCheck {
            name: check.name.clone(),
            kind: check.kind,
            passed: output.status.success(),
            details: format!(
                "exit_code={:?}; stdout_bytes={}; stderr_bytes={}",
                output.status.code(),
                output.stdout.len(),
                output.stderr.len()
            ),
        });
    }
    GraderOutcome {
        success,
        exit_code,
        stdout,
        stderr,
        duration_ms,
        checks,
    }
}

fn ensure_docker_started(output: &BoundedOutput) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let runtime_error = matches!(output.status.code(), Some(125..=127))
        || stderr.contains("Docker daemon")
        || stderr.contains("docker daemon")
        || stderr.contains("Error response from daemon")
        || stderr.contains("No such image")
        || stderr.contains("pull access denied");
    anyhow::ensure!(
        !runtime_error,
        "Docker verifier failed before the grader ran: {}",
        stderr.trim()
    );
    Ok(())
}

fn copy_toolchain_environment(command: &mut Command) {
    for variable in [
        "PATH",
        "RUSTUP_HOME",
        "CARGO_HOME",
        "RUSTUP_TOOLCHAIN",
        "RUSTC",
        "RUSTDOC",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NIX_SSL_CERT_FILE",
    ] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    if std::env::var_os("RUSTUP_HOME").is_none() {
        for variable in ["HOME", "USERPROFILE"] {
            let Some(home) = std::env::var_os(variable) else {
                continue;
            };
            let rustup_home = Path::new(&home).join(".rustup");
            if rustup_home.is_dir() {
                command.env("RUSTUP_HOME", rustup_home);
                break;
            }
        }
    }
    copy_platform_toolchain_environment(command);
}

#[cfg(windows)]
fn copy_platform_toolchain_environment(command: &mut Command) {
    for variable in [
        "SystemRoot",
        "WINDIR",
        "SYSTEMDRIVE",
        "COMSPEC",
        "PATHEXT",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "VCINSTALLDIR",
        "VCToolsInstallDir",
        "VCToolsVersion",
        "VCToolsRedistDir",
        "VSINSTALLDIR",
        "WindowsSdkDir",
        "WindowsSDKVersion",
        "UniversalCRTSdkDir",
        "UCRTVersion",
    ] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }

    if let Some(linker) = find_msvc_tools::find_tool("x86_64-pc-windows-msvc", "link.exe") {
        command.env("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER", linker.path());
        for (variable, value) in linker.env() {
            command.env(variable, value);
        }
    }
}

#[cfg(not(windows))]
fn copy_platform_toolchain_environment(_: &mut Command) {}

pub(crate) async fn force_remove_container(name: &str) {
    let mut command = Command::new("docker");
    command
        .args(["rm", "--force", name])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    let _ = command.status().await;
}

#[cfg(unix)]
pub(crate) fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
pub(crate) fn configure_process_group(_: &mut Command) {}

#[cfg(unix)]
async fn terminate_process_tree(child: &mut Child, pid: Option<u32>) {
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(not(unix))]
async fn terminate_process_tree(child: &mut Child, _: Option<u32>) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(unix)]
fn non_root_user_spec(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.uid() != 0 => format!("{}:{}", metadata.uid(), metadata.gid()),
        _ => "1000:1000".into(),
    }
}

#[cfg(not(unix))]
fn non_root_user_spec(_: &Path) -> String {
    "1000:1000".into()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fast_output_burst_cannot_race_past_limit() {
        for _ in 0..16 {
            let mut command = Command::new("sh");
            command
                .args([
                    "-c",
                    "i=0; while [ \"$i\" -lt 64 ]; do printf 0123456789abcdef; i=$((i + 1)); done",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let error = match run_bounded(command, std::time::Duration::from_secs(5), 32).await {
                Ok(_) => panic!("fast oversized output was accepted"),
                Err(error) => error,
            };

            assert!(error.to_string().contains("output limit"));
        }
    }
}
