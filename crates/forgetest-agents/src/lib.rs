//! External coding-agent adapters with bounded, noninteractive execution.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use forgetest_core::agent::{
    AgentEvent, AgentEventKind, AgentExecutor, AgentIdentity, AgentOutcome, AgentRequest,
    AgentTerminationReason, AgentUsage, EventSink, WorkspaceEnvironment,
};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Notify};

const MAX_NORMALIZED_EVENTS: u64 = 10_000;

/// Immutable benchmark selections recorded before a published study.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkLock {
    pub schema_version: u32,
    pub created_at: chrono::DateTime<Utc>,
    pub suite_digest: String,
    pub policy_digest: String,
    pub verifier_image: String,
    pub agents: Vec<LockedAgent>,
}

/// Exact executable and model selection for one external agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedAgent {
    pub name: String,
    pub model: String,
    pub cli_version: String,
    pub executable_sha256: String,
    pub configuration_digest: String,
    pub container_image: String,
    #[serde(default)]
    pub effort: Option<String>,
}

/// Executable identity observed inside a locked agent image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDoctorReport {
    pub executable_path: String,
    pub executable_sha256: String,
    pub version: String,
}

impl LockedAgent {
    pub fn verify_container(&self, observed: &ContainerDoctorReport) -> Result<()> {
        anyhow::ensure!(
            self.cli_version == observed.version,
            "agent '{}' CLI version differs from benchmark lock: expected '{}', observed '{}'",
            self.name,
            self.cli_version,
            observed.version
        );
        anyhow::ensure!(
            self.executable_sha256
                .eq_ignore_ascii_case(&observed.executable_sha256),
            "agent '{}' executable SHA-256 differs from benchmark lock",
            self.name
        );
        Ok(())
    }

    pub fn verify_profile(&self, profile: &CommandProfile) -> Result<()> {
        let observed = profile_configuration_digest(profile, &self.container_image);
        anyhow::ensure!(
            self.configuration_digest.eq_ignore_ascii_case(&observed),
            "agent '{}' command profile differs from benchmark lock",
            self.name
        );
        Ok(())
    }
}

impl BenchmarkLock {
    pub fn parse(content: &str) -> Result<Self> {
        let lock: Self = toml::from_str(content).context("failed to parse benchmark lock")?;
        lock.validate_structure()?;
        Ok(lock)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read benchmark lock: {}", path.display()))?;
        Self::parse(&content)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate_structure()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("failed to write benchmark lock: {}", path.display()))
    }

    pub fn agent(&self, name: &str) -> Result<&LockedAgent> {
        self.agents
            .iter()
            .find(|agent| agent.name == name)
            .with_context(|| format!("agent '{name}' is not present in benchmark lock"))
    }

    pub fn validate(
        &self,
        suite_digest: &str,
        policy_digest: &str,
        requested_agents: &[&str],
    ) -> Result<()> {
        self.validate_structure()?;
        anyhow::ensure!(
            self.suite_digest == suite_digest,
            "benchmark lock suite digest does not match loaded suite"
        );
        anyhow::ensure!(
            self.policy_digest == policy_digest,
            "benchmark lock policy digest does not match execution policy"
        );
        let mut requested = std::collections::HashSet::new();
        for name in requested_agents {
            anyhow::ensure!(requested.insert(*name), "duplicate requested agent: {name}");
            self.agent(name)?;
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<()> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported benchmark lock schema version: {}",
            self.schema_version
        );
        anyhow::ensure!(
            is_sha256_hex(&self.suite_digest),
            "lock suite digest must be a 64-character SHA-256"
        );
        anyhow::ensure!(
            is_sha256_hex(&self.policy_digest),
            "lock policy digest must be a 64-character SHA-256"
        );
        anyhow::ensure!(
            is_immutable_image(&self.verifier_image),
            "benchmark verifier image must use an immutable sha256 digest"
        );
        anyhow::ensure!(!self.agents.is_empty(), "benchmark lock contains no agents");
        let mut names = std::collections::HashSet::new();
        for agent in &self.agents {
            anyhow::ensure!(
                names.insert(agent.name.as_str()),
                "duplicate locked agent: {}",
                agent.name
            );
            anyhow::ensure!(
                is_exact_model_selection(&agent.model),
                "locked agent '{}' must use an exact model ID, not an alias",
                agent.name
            );
            anyhow::ensure!(
                !agent.cli_version.is_empty(),
                "locked agent CLI version is empty"
            );
            anyhow::ensure!(
                is_sha256_hex(&agent.executable_sha256),
                "locked agent executable_sha256 is invalid"
            );
            anyhow::ensure!(
                is_sha256_hex(&agent.configuration_digest),
                "locked agent configuration digest must be a 64-character SHA-256"
            );
            anyhow::ensure!(
                is_immutable_image(&agent.container_image),
                "locked agent '{}' must use an immutable container image",
                agent.name
            );
            if let Some(effort) = &agent.effort {
                anyhow::ensure!(
                    !effort.is_empty()
                        && effort.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
                        }),
                    "locked agent '{}' effort is invalid",
                    agent.name
                );
            }
        }
        Ok(())
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_exact_model_selection(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    let has_moving_alias = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| matches!(token, "default" | "latest" | "auto" | "recommended"));
    !normalized.is_empty()
        && normalized == value.to_ascii_lowercase()
        && !value.chars().any(char::is_whitespace)
        && !has_moving_alias
        && !matches!(
            normalized.as_str(),
            "default" | "latest" | "auto" | "recommended" | "sonnet" | "opus" | "haiku" | "fable"
        )
}

/// Whether an OCI image reference includes a complete immutable SHA-256.
pub fn is_immutable_image(value: &str) -> bool {
    let Some((name, digest)) = value.rsplit_once("@sha256:") else {
        return false;
    };
    !name.is_empty() && !name.contains(char::is_whitespace) && is_sha256_hex(digest)
}

/// Parser used for one external agent's output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventParser {
    CodexJsonl,
    ClaudeStreamJson,
    GenericJsonl,
    Text,
}

/// Noninteractive command profile for an external agent.
#[derive(Debug, Clone)]
pub struct CommandProfile {
    pub name: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub version_arguments: Vec<String>,
    pub parser: EventParser,
    pub environment_allowlist: Vec<String>,
}

impl CommandProfile {
    pub fn codex(model: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            name: "codex".into(),
            executable: "codex".into(),
            arguments: vec![
                "exec".into(),
                "--json".into(),
                "--ephemeral".into(),
                "--sandbox".into(),
                "workspace-write".into(),
                "--ignore-user-config".into(),
                "--ignore-rules".into(),
                "--model".into(),
                model,
                "-C".into(),
                "{workspace}".into(),
                "-".into(),
            ],
            version_arguments: vec!["--version".into()],
            parser: EventParser::CodexJsonl,
            environment_allowlist: vec!["OPENAI_API_KEY".into()],
        }
    }

    pub fn claude(model: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            name: "claude".into(),
            executable: "claude".into(),
            arguments: vec![
                "--print".into(),
                "--bare".into(),
                "--output-format".into(),
                "stream-json".into(),
                "--no-session-persistence".into(),
                "--permission-mode".into(),
                "bypassPermissions".into(),
                "--model".into(),
                model,
            ],
            version_arguments: vec!["--version".into()],
            parser: EventParser::ClaudeStreamJson,
            environment_allowlist: vec!["ANTHROPIC_API_KEY".into()],
        }
    }

    pub fn generic(executable: impl ToString, parser: EventParser) -> Self {
        Self {
            name: "generic".into(),
            executable: executable.to_string(),
            arguments: Vec::new(),
            version_arguments: vec!["--version".into()],
            parser,
            environment_allowlist: Vec::new(),
        }
    }

    fn rendered_arguments(&self, workspace: &Path) -> Vec<String> {
        let workspace = workspace.to_string_lossy();
        self.arguments
            .iter()
            .map(|argument| argument.replace("{workspace}", &workspace))
            .collect()
    }
}

/// Build one of the supported external-agent profiles with an explicit effort.
pub fn builtin_profile(name: &str, model: &str, effort: Option<&str>) -> Result<CommandProfile> {
    anyhow::ensure!(!model.trim().is_empty(), "agent model is empty");
    let mut profile = match name {
        "codex" => CommandProfile::codex(model),
        "claude" => CommandProfile::claude(model),
        other => anyhow::bail!("unsupported agent: {other}"),
    };
    if let Some(effort) = effort {
        anyhow::ensure!(
            !effort.is_empty()
                && effort
                    .bytes()
                    .all(|byte| { byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') }),
            "agent effort must be an ASCII identifier"
        );
        match name {
            "codex" => {
                profile.arguments.splice(
                    1..1,
                    ["-c".into(), format!("model_reasoning_effort=\"{effort}\"")],
                );
            }
            "claude" => {
                profile.arguments.push("--effort".into());
                profile.arguments.push(effort.into());
            }
            _ => unreachable!("agent name was validated"),
        }
    }
    Ok(profile)
}

/// SHA-256 identity for all command-profile inputs that can affect a trial.
pub fn profile_configuration_digest(profile: &CommandProfile, container_image: &str) -> String {
    let parser = match profile.parser {
        EventParser::CodexJsonl => "codex_jsonl",
        EventParser::ClaudeStreamJson => "claude_stream_json",
        EventParser::GenericJsonl => "generic_jsonl",
        EventParser::Text => "text",
    };
    let material = (
        "forgetest-agent-profile-v1",
        &profile.name,
        &profile.executable,
        &profile.arguments,
        &profile.version_arguments,
        parser,
        &profile.environment_allowlist,
        container_image,
    );
    let encoded =
        serde_json::to_vec(&material).expect("command profile contains serializable values");
    hex_digest(Sha256::digest(encoded))
}

/// Thread-safe in-memory sink used by tests and simple callers.
#[derive(Default)]
pub struct MemoryEventSink {
    events: Mutex<Vec<AgentEvent>>,
}

impl MemoryEventSink {
    pub fn events(&self) -> Vec<AgentEvent> {
        self.events.lock().expect("event lock poisoned").clone()
    }
}

impl EventSink for MemoryEventSink {
    fn emit(&self, event: &AgentEvent) -> Result<()> {
        self.events
            .lock()
            .map_err(|_| anyhow::anyhow!("event lock poisoned"))?
            .push(event.clone());
        Ok(())
    }
}

/// One deterministic file replacement made by the scripted adapter.
#[derive(Debug, Clone)]
pub struct ScriptedEdit {
    pub path: PathBuf,
    pub content: String,
}

/// Deterministic no-key agent used by CI and the repository demo.
pub struct ScriptedAgent {
    identity: AgentIdentity,
    edits: Vec<ScriptedEdit>,
}

impl ScriptedAgent {
    pub fn new(identity: AgentIdentity, edits: Vec<ScriptedEdit>) -> Self {
        Self { identity, edits }
    }
}

#[async_trait]
impl AgentExecutor for ScriptedAgent {
    fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        let start = Instant::now();
        let mut emitted = Vec::new();
        emit(
            events,
            &mut emitted,
            AgentEventKind::Started,
            format!("started task {}", request.task_id),
            None,
        )?;
        let workspace = request.workspace.canonicalize().with_context(|| {
            format!("workspace does not exist: {}", request.workspace.display())
        })?;

        for edit in &self.edits {
            let destination = safe_destination(&workspace, &edit.path)?;
            if destination
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                anyhow::bail!(
                    "scripted edit destination is a symlink: {}",
                    edit.path.display()
                );
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
                let resolved_parent = parent.canonicalize()?;
                anyhow::ensure!(
                    resolved_parent.starts_with(&workspace),
                    "scripted edit path escapes workspace: {}",
                    edit.path.display()
                );
            }
            std::fs::write(&destination, &edit.content)
                .with_context(|| format!("failed to write {}", edit.path.display()))?;
            emit(
                events,
                &mut emitted,
                AgentEventKind::ToolResult,
                format!("wrote {}", edit.path.display()),
                None,
            )?;
        }

        emit(
            events,
            &mut emitted,
            AgentEventKind::Completed,
            "scripted agent completed".into(),
            None,
        )?;
        Ok(AgentOutcome {
            identity: self.identity.clone(),
            termination: AgentTerminationReason::Completed,
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis() as u64,
            usage: AgentUsage::default(),
            events: emitted,
            error: None,
        })
    }
}

/// Executes a command profile in an isolated process environment.
pub struct ProcessAgent {
    profile: CommandProfile,
    identity: AgentIdentity,
}

/// Resource and network policy for an outer agent container.
#[derive(Debug, Clone)]
pub struct DockerAgentConfig {
    pub image: String,
    pub network: String,
    pub memory: String,
    pub cpus: f64,
    pub pids_limit: u32,
    pub tmpfs_size: String,
}

impl Default for DockerAgentConfig {
    fn default() -> Self {
        Self {
            image: "forgetest-agent:0.1.0".into(),
            network: "bridge".into(),
            memory: "2g".into(),
            cpus: 2.0,
            pids_limit: 256,
            tmpfs_size: "256m".into(),
        }
    }
}

/// Runs a known external agent CLI inside an ephemeral outer container.
pub struct DockerProcessAgent {
    profile: CommandProfile,
    identity: AgentIdentity,
    config: DockerAgentConfig,
}

impl DockerProcessAgent {
    pub fn new(
        profile: CommandProfile,
        identity: AgentIdentity,
        config: DockerAgentConfig,
    ) -> Self {
        Self {
            profile,
            identity,
            config,
        }
    }

    pub fn docker_arguments(&self, container_name: &str, workspace: &Path) -> Vec<String> {
        let mount = format!("type=bind,src={},dst=/work", workspace.to_string_lossy());
        let mut arguments = vec![
            "run".into(),
            "--rm".into(),
            "--interactive".into(),
            "--name".into(),
            container_name.into(),
            "--network".into(),
            self.config.network.clone(),
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
            mount,
            "--tmpfs".into(),
            format!("/tmp:rw,nosuid,size={}", self.config.tmpfs_size),
            "--workdir".into(),
            "/work".into(),
            "--env".into(),
            "HOME=/tmp/home".into(),
        ];
        for variable in &self.profile.environment_allowlist {
            arguments.push("--env".into());
            arguments.push(variable.clone());
        }
        arguments.push(self.config.image.clone());
        arguments.push(self.profile.executable.clone());
        arguments.extend(
            self.profile
                .arguments
                .iter()
                .map(|argument| argument.replace("{workspace}", "/work")),
        );
        arguments
    }
}

/// Build a credential-free command that inspects an agent executable inside
/// its immutable container image.
pub fn docker_preflight_arguments(profile: &CommandProfile, image: &str) -> Vec<String> {
    let mut arguments = vec![
        "run".into(),
        "--rm".into(),
        "--network".into(),
        "none".into(),
        "--read-only".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--security-opt".into(),
        "no-new-privileges".into(),
        "--user".into(),
        "65532:65532".into(),
        "--tmpfs".into(),
        "/tmp:rw,noexec,nosuid,size=16m".into(),
        image.into(),
        "sh".into(),
        "-c".into(),
        concat!(
            "binary=$(command -v \"$1\") || exit 64; ",
            "printf 'path=%s\\n' \"$binary\"; ",
            "sha256sum \"$binary\"; ",
            "shift; \"$binary\" \"$@\""
        )
        .into(),
        "forgetest-preflight".into(),
        profile.executable.clone(),
    ];
    arguments.extend(profile.version_arguments.iter().cloned());
    arguments
}

/// Inspect an agent CLI inside its container without credentials or network.
pub async fn doctor_container(
    profile: &CommandProfile,
    image: &str,
) -> Result<ContainerDoctorReport> {
    let workspace = tempfile::tempdir().context("failed to create preflight workspace")?;
    let docker_profile = CommandProfile {
        name: format!("docker-preflight-{}", profile.name),
        executable: "docker".into(),
        arguments: docker_preflight_arguments(profile, image),
        version_arguments: vec!["--version".into()],
        parser: EventParser::Text,
        environment_allowlist: vec!["DOCKER_HOST".into()],
    };
    let process = ProcessAgent::new(
        docker_profile,
        AgentIdentity {
            adapter: format!("preflight-{}", profile.name),
            adapter_version: "1".into(),
            executable_sha256: None,
            model: "none".into(),
            configuration_digest: "credential-free-container-preflight-v1".into(),
        },
    );
    let outcome = process
        .execute(
            &AgentRequest {
                trial_id: uuid::Uuid::new_v4(),
                task_id: "container-preflight".into(),
                prompt: String::new(),
                workspace: workspace.path().to_path_buf(),
                limits: forgetest_core::agent::AgentLimits {
                    timeout_secs: 30,
                    max_output_bytes: 64 * 1024,
                    ..forgetest_core::agent::AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await?;
    anyhow::ensure!(
        outcome.termination == AgentTerminationReason::Completed,
        "agent container preflight failed: {}",
        outcome
            .error
            .as_deref()
            .unwrap_or("container command did not complete")
    );
    let output = outcome
        .events
        .iter()
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    parse_container_doctor_output(output.as_bytes())
}

/// Verify that an immutable verifier image exposes a working Cargo executable.
pub async fn doctor_verifier_container(image: &str) -> Result<ContainerDoctorReport> {
    let profile = CommandProfile::generic("cargo", EventParser::Text);
    let observed = doctor_container(&profile, image).await?;
    let executable = Path::new(&observed.executable_path)
        .file_name()
        .and_then(|name| name.to_str());
    anyhow::ensure!(
        executable == Some("cargo"),
        "verifier image resolved an unexpected Cargo executable: {}",
        observed.executable_path
    );
    Ok(observed)
}

/// Parse the stable output contract emitted by the container preflight.
pub fn parse_container_doctor_output(output: &[u8]) -> Result<ContainerDoctorReport> {
    let text = std::str::from_utf8(output).context("container preflight output is not UTF-8")?;
    let mut lines = text.lines();
    let executable_path = lines
        .next()
        .and_then(|line| line.strip_prefix("path="))
        .context("container preflight did not report an executable path")?
        .trim()
        .to_string();
    anyhow::ensure!(
        !executable_path.is_empty(),
        "container preflight executable path is empty"
    );
    let executable_sha256 = lines
        .next()
        .and_then(|line| line.split_whitespace().next())
        .context("container preflight did not report executable SHA-256")?
        .to_ascii_lowercase();
    anyhow::ensure!(
        executable_sha256.len() == 64
            && executable_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "container preflight reported an invalid executable SHA-256"
    );
    let version = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    anyhow::ensure!(
        !version.is_empty(),
        "container preflight did not report a CLI version"
    );
    Ok(ContainerDoctorReport {
        executable_path,
        executable_sha256,
        version,
    })
}

#[async_trait]
impl AgentExecutor for DockerProcessAgent {
    fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        let container_name = format!("forgetest-agent-{}", request.trial_id.simple());
        let mut outer_environment = self.profile.environment_allowlist.clone();
        outer_environment.push("DOCKER_HOST".into());
        let docker_profile = CommandProfile {
            name: format!("docker-{}", self.profile.name),
            executable: "docker".into(),
            arguments: self.docker_arguments(&container_name, &request.workspace),
            version_arguments: vec!["--version".into()],
            parser: self.profile.parser,
            environment_allowlist: outer_environment,
        };
        let process = ProcessAgent::new(docker_profile, self.identity.clone());
        let result = process.execute(request, events).await;
        if result.as_ref().map_or(true, |outcome| {
            outcome.termination != AgentTerminationReason::Completed
        }) {
            force_remove_container(&container_name).await;
        }
        result
    }
}

/// Trusted host environment used for local development and offline demos.
pub struct DirectWorkspaceEnvironment;

#[async_trait]
impl WorkspaceEnvironment for DirectWorkspaceEnvironment {
    fn identity(&self) -> String {
        "host-trusted".into()
    }

    async fn execute(
        &self,
        agent: &dyn AgentExecutor,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        agent.execute(request, events).await
    }
}

impl ProcessAgent {
    pub fn new(profile: CommandProfile, identity: AgentIdentity) -> Self {
        Self { profile, identity }
    }
}

#[async_trait]
impl AgentExecutor for ProcessAgent {
    fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        anyhow::ensure!(
            request.limits.timeout_secs > 0,
            "agent timeout must be positive"
        );
        anyhow::ensure!(
            request.limits.max_output_bytes > 0,
            "agent output limit must be positive"
        );
        let start = Instant::now();
        let isolated_home = tempfile::tempdir().context("failed to create isolated agent home")?;
        let mut command = Command::new(&self.profile.executable);
        command
            .args(self.profile.rendered_arguments(&request.workspace))
            .current_dir(&request.workspace)
            .env_clear()
            .env("HOME", isolated_home.path())
            .env("USERPROFILE", isolated_home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        for variable in &self.profile.environment_allowlist {
            if let Some(value) = std::env::var_os(variable) {
                command.env(variable, value);
            }
        }
        configure_process_group(&mut command);

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start {}", self.profile.executable))?;
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .context("agent stdout was not captured")?;
        let stderr = child
            .stderr
            .take()
            .context("agent stderr was not captured")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(request.prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let used = Arc::new(AtomicUsize::new(0));
        let exceeded = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let stdout_task = tokio::spawn(collect_bounded_streaming(
            stdout,
            request.limits.max_output_bytes,
            Arc::clone(&used),
            Arc::clone(&exceeded),
            Arc::clone(&notify),
            self.profile.parser,
            event_sender,
        ));
        let stderr_task = tokio::spawn(collect_bounded(
            stderr,
            request.limits.max_output_bytes,
            Arc::clone(&used),
            Arc::clone(&exceeded),
            Arc::clone(&notify),
        ));

        enum Stop {
            Exited(std::process::ExitStatus),
            Timeout,
            OutputLimit,
            EventSink(anyhow::Error),
        }
        let timeout =
            tokio::time::sleep(std::time::Duration::from_secs(request.limits.timeout_secs));
        tokio::pin!(timeout);
        let mut parsed_events = Vec::new();
        let mut event_stream_open = true;
        let stop = loop {
            tokio::select! {
                status = child.wait() => {
                    break Stop::Exited(status.context("failed waiting for agent")?);
                }
                _ = &mut timeout => break Stop::Timeout,
                _ = notify.notified() => break Stop::OutputLimit,
                event = event_receiver.recv(), if event_stream_open => {
                    match event {
                        Some(event) => {
                            if let Err(error) = events.emit(&event) {
                                break Stop::EventSink(error);
                            }
                            parsed_events.push(event);
                        }
                        None => event_stream_open = false,
                    }
                }
            }
        };

        if !matches!(&stop, Stop::Exited(_)) {
            terminate_process_tree(&mut child, pid).await;
        }
        let _stdout = stdout_task.await.context("stdout collector failed")??;
        let stderr = stderr_task.await.context("stderr collector failed")??;
        while let Ok(event) = event_receiver.try_recv() {
            events.emit(&event)?;
            parsed_events.push(event);
        }
        if !stderr.is_empty() {
            if parsed_events.len() as u64 >= MAX_NORMALIZED_EVENTS {
                exceeded.store(true, Ordering::SeqCst);
            } else {
                let event = AgentEvent {
                    sequence: parsed_events.len() as u64 + 1,
                    timestamp: Utc::now(),
                    kind: AgentEventKind::Warning,
                    message: String::from_utf8_lossy(&stderr).trim().to_string(),
                    raw: None,
                };
                events.emit(&event)?;
                parsed_events.push(event);
            }
        }
        let output_exceeded = exceeded.load(Ordering::SeqCst);
        if let Stop::EventSink(error) = &stop {
            anyhow::bail!("agent event sink rejected a streamed event: {error:#}");
        }
        let usage = usage_from_events(&parsed_events);
        let observed_exit_code = match &stop {
            Stop::Exited(status) => status.code(),
            Stop::Timeout | Stop::OutputLimit | Stop::EventSink(_) => None,
        };
        let budget_exceeded = request
            .limits
            .max_tokens
            .is_some_and(|limit| usage.input_tokens + usage.output_tokens > limit)
            || request
                .limits
                .max_cost_usd
                .is_some_and(|limit| usage.estimated_cost_usd > limit);

        let (termination, exit_code, error) = if output_exceeded {
            (
                AgentTerminationReason::OutputLimit,
                observed_exit_code,
                Some("agent exceeded output limit".into()),
            )
        } else if budget_exceeded {
            (
                AgentTerminationReason::BudgetExceeded,
                observed_exit_code,
                Some("agent-reported usage exceeded configured budget".into()),
            )
        } else {
            match stop {
                Stop::Exited(status) if status.success() => {
                    (AgentTerminationReason::Completed, status.code(), None)
                }
                Stop::Exited(status) => (
                    AgentTerminationReason::ExitNonZero,
                    status.code(),
                    Some(String::from_utf8_lossy(&stderr).trim().to_string()),
                ),
                Stop::Timeout => (
                    AgentTerminationReason::Timeout,
                    None,
                    Some("agent timed out".into()),
                ),
                Stop::OutputLimit => (
                    AgentTerminationReason::OutputLimit,
                    None,
                    Some("agent exceeded output limit".into()),
                ),
                Stop::EventSink(_) => unreachable!("event sink errors return above"),
            }
        };

        Ok(AgentOutcome {
            identity: self.identity.clone(),
            termination,
            exit_code,
            duration_ms: start.elapsed().as_millis() as u64,
            usage,
            events: parsed_events,
            error,
        })
    }
}

/// Result of a non-secret agent installation and authentication preflight.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub profile: String,
    pub executable_found: bool,
    pub executable_path: Option<PathBuf>,
    pub executable_sha256: Option<String>,
    pub version: Option<String>,
    pub available_credentials: Vec<String>,
    pub missing_credentials: Vec<String>,
}

/// Inspect an external agent without printing credential values.
pub fn doctor(profile: &CommandProfile) -> Result<DoctorReport> {
    let executable_path = resolve_executable(&profile.executable);
    let executable_found = executable_path.is_some();
    let executable_sha256 = executable_path.as_deref().map(file_sha256).transpose()?;
    let version = executable_path.as_deref().and_then(|path| {
        let mut command = std::process::Command::new(path);
        command
            .args(&profile.version_arguments)
            .env_clear()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(value) = std::env::var_os("PATH") {
            command.env("PATH", value);
        }
        command.output().ok().and_then(|output| {
            output.status.success().then(|| {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    String::from_utf8_lossy(&output.stderr).trim().to_string()
                } else {
                    stdout
                }
            })
        })
    });
    let (available_credentials, missing_credentials) = profile
        .environment_allowlist
        .iter()
        .cloned()
        .partition(|variable| std::env::var_os(variable).is_some());

    Ok(DoctorReport {
        profile: profile.name.clone(),
        executable_found,
        executable_path,
        executable_sha256,
        version,
        available_credentials,
        missing_credentials,
    })
}

fn emit(
    sink: &dyn EventSink,
    events: &mut Vec<AgentEvent>,
    kind: AgentEventKind,
    message: String,
    raw: Option<serde_json::Value>,
) -> Result<()> {
    let event = AgentEvent {
        sequence: events.len() as u64 + 1,
        timestamp: Utc::now(),
        kind,
        message,
        raw,
    };
    sink.emit(&event)?;
    events.push(event);
    Ok(())
}

fn safe_destination(workspace: &Path, relative: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_) | Component::CurDir)),
        "edit path must be relative and cannot contain '..': {}",
        relative.display()
    );
    Ok(workspace.join(relative))
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

async fn collect_bounded_streaming<R>(
    mut reader: R,
    limit: usize,
    used: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
    notify: Arc<Notify>,
    parser: EventParser,
    sender: mpsc::UnboundedSender<AgentEvent>,
) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut collected = Vec::new();
    let mut line = Vec::new();
    let mut sequence = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let previous = used.fetch_add(read, Ordering::Relaxed);
        let keep = if previous < limit {
            read.min(limit - previous)
        } else {
            0
        };
        collected.extend_from_slice(&buffer[..keep]);
        for byte in &buffer[..keep] {
            if *byte == b'\n' {
                if !send_parsed_line(
                    &line,
                    parser,
                    &mut sequence,
                    &sender,
                    exceeded.as_ref(),
                    notify.as_ref(),
                ) {
                    return Ok(collected);
                }
                line.clear();
            } else {
                line.push(*byte);
            }
        }
        if previous.saturating_add(read) > limit && !exceeded.swap(true, Ordering::SeqCst) {
            notify.notify_one();
        }
    }
    if !line.is_empty() {
        send_parsed_line(
            &line,
            parser,
            &mut sequence,
            &sender,
            exceeded.as_ref(),
            notify.as_ref(),
        );
    }
    Ok(collected)
}

fn send_parsed_line(
    line: &[u8],
    parser: EventParser,
    sequence: &mut u64,
    sender: &mpsc::UnboundedSender<AgentEvent>,
    exceeded: &AtomicBool,
    notify: &Notify,
) -> bool {
    for mut event in parse_events(line, parser) {
        if *sequence >= MAX_NORMALIZED_EVENTS {
            if !exceeded.swap(true, Ordering::SeqCst) {
                notify.notify_one();
            }
            return false;
        }
        *sequence += 1;
        event.sequence = *sequence;
        if sender.send(event).is_err() {
            return false;
        }
    }
    true
}

fn parse_events(output: &[u8], parser: EventParser) -> Vec<AgentEvent> {
    let text = String::from_utf8_lossy(output);
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            if parser == EventParser::Text {
                return AgentEvent {
                    sequence: index as u64 + 1,
                    timestamp: Utc::now(),
                    kind: AgentEventKind::Message,
                    message: line.to_string(),
                    raw: None,
                };
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(raw) => {
                    let event_type = raw
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    let kind = classify_event(event_type);
                    let message = event_message(&raw).unwrap_or_else(|| event_type.to_string());
                    AgentEvent {
                        sequence: index as u64 + 1,
                        timestamp: Utc::now(),
                        kind,
                        message,
                        raw: Some(raw),
                    }
                }
                Err(_) => AgentEvent {
                    sequence: index as u64 + 1,
                    timestamp: Utc::now(),
                    kind: AgentEventKind::Unknown,
                    message: line.to_string(),
                    raw: None,
                },
            }
        })
        .collect()
}

fn classify_event(event_type: &str) -> AgentEventKind {
    let lower = event_type.to_ascii_lowercase();
    if lower.contains("start") || lower == "system" {
        AgentEventKind::Started
    } else if lower.contains("tool") && lower.contains("result") {
        AgentEventKind::ToolResult
    } else if lower.contains("tool") {
        AgentEventKind::ToolCall
    } else if lower.contains("usage") {
        AgentEventKind::Usage
    } else if lower.contains("error") {
        AgentEventKind::Error
    } else if lower.contains("complete") || lower == "result" {
        AgentEventKind::Completed
    } else if lower.contains("message") || lower.contains("assistant") {
        AgentEventKind::Message
    } else {
        AgentEventKind::Unknown
    }
}

fn event_message(value: &serde_json::Value) -> Option<String> {
    ["message", "text", "content", "result"]
        .iter()
        .find_map(|key| match value.get(*key) {
            Some(serde_json::Value::String(message)) => Some(message.clone()),
            Some(other) if !other.is_null() => Some(other.to_string()),
            _ => None,
        })
}

fn usage_from_events(events: &[AgentEvent]) -> AgentUsage {
    let mut usage = AgentUsage::default();
    for raw in events.iter().filter_map(|event| event.raw.as_ref()) {
        let source = raw.get("usage").unwrap_or(raw);
        usage.input_tokens = usage.input_tokens.max(
            source
                .get("input_tokens")
                .or_else(|| source.get("inputTokens"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        );
        usage.output_tokens = usage.output_tokens.max(
            source
                .get("output_tokens")
                .or_else(|| source.get("outputTokens"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        );
        usage.cached_tokens = usage.cached_tokens.max(
            source
                .get("cached_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        );
        usage.estimated_cost_usd = usage.estimated_cost_usd.max(
            source
                .get("estimated_cost_usd")
                .or_else(|| source.get("total_cost_usd"))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0),
        );
    }
    usage
}

fn resolve_executable(executable: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(executable);
    if candidate.is_absolute() || candidate.components().count() > 1 {
        return candidate.is_file().then_some(candidate);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(executable))
        .find(|path| path.is_file())
}

async fn force_remove_container(name: &str) {
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
    if let Some(host) = std::env::var_os("DOCKER_HOST") {
        command.env("DOCKER_HOST", host);
    }
    let _ = command.status().await;
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
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

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_: &mut Command) {}

#[cfg(unix)]
async fn terminate_process_tree(child: &mut Child, pid: Option<u32>) {
    if let Some(pid) = pid {
        // The child is placed in its own process group before spawn.
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
