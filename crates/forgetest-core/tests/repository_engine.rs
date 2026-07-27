use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use forgetest_core::agent::{
    AgentEvent, AgentEventKind, AgentExecutor, AgentIdentity, AgentOutcome, AgentRequest,
    AgentTerminationReason, AgentUsage, EventSink, GradeRequest, Grader, WorkspaceEnvironment,
};
use forgetest_core::repository_engine::{RepositoryEngine, RepositoryEngineConfig};
use forgetest_core::repository_report::{
    ExecutionPolicyManifest, GraderCheck, GraderCheckKind, GraderOutcome, TrialStatus,
};
use forgetest_core::suite::load_suite;

fn write_suite(root: &Path) {
    std::fs::create_dir_all(root.join("tasks/fix/workspace/src")).unwrap();
    std::fs::create_dir_all(root.join("tasks/fix/grader/tests")).unwrap();
    std::fs::write(
        root.join("suite.toml"),
        r#"
schema_version = 2
id = "suite"
name = "Suite"
[[tasks]]
id = "fix"
path = "tasks/fix"
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix/task.toml"),
        r#"
schema_version = 1
id = "fix"
name = "Fix"
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
timeout_secs = 30
[verifier]
command = ["cargo", "test", "--locked"]
timeout_secs = 30
[provenance]
kind = "authored"
license = "MIT"
"#,
    )
    .unwrap();
    std::fs::write(root.join("tasks/fix/prompt.md"), "Return 42.").unwrap();
    std::fs::write(
        root.join("tasks/fix/workspace/Cargo.toml"),
        "[package]\nname=\"fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix/workspace/src/lib.rs"),
        "pub fn value() -> u8 { 0 }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tasks/fix/grader/tests/hidden.rs"),
        "#[test] fn hidden() {}\n",
    )
    .unwrap();
}

fn identity() -> AgentIdentity {
    AgentIdentity {
        adapter: "editing".into(),
        adapter_version: "1".into(),
        executable_sha256: Some("binary".into()),
        model: "deterministic".into(),
        configuration_digest: "config".into(),
    }
}

struct EditingAgent {
    fail: bool,
}

#[async_trait]
impl AgentExecutor for EditingAgent {
    fn identity(&self) -> &AgentIdentity {
        static IDENTITY: std::sync::OnceLock<AgentIdentity> = std::sync::OnceLock::new();
        IDENTITY.get_or_init(identity)
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        assert!(!request.workspace.join("grader").exists());
        if self.fail {
            anyhow::bail!("agent crashed");
        }
        std::fs::write(
            request.workspace.join("src/lib.rs"),
            "pub fn value() -> u8 { 42 }\n",
        )?;
        let event = AgentEvent {
            sequence: 1,
            timestamp: chrono::Utc::now(),
            kind: AgentEventKind::ToolResult,
            message: "updated src/lib.rs".into(),
            raw: None,
        };
        events.emit(&event)?;
        Ok(AgentOutcome {
            identity: identity(),
            termination: AgentTerminationReason::Completed,
            exit_code: Some(0),
            duration_ms: 1,
            usage: AgentUsage::default(),
            events: vec![event],
            error: None,
        })
    }
}

struct ConcurrencyAgent {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

struct FlakyAgent {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl AgentExecutor for FlakyAgent {
    fn identity(&self) -> &AgentIdentity {
        static IDENTITY: std::sync::OnceLock<AgentIdentity> = std::sync::OnceLock::new();
        IDENTITY.get_or_init(identity)
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        _events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let source = request.workspace.join("src/lib.rs");
        if call == 0 {
            std::fs::write(&source, "pub fn value() -> u8 { 99 }\n")?;
            anyhow::bail!("transient agent failure");
        }

        let restored = std::fs::read_to_string(&source)?;
        anyhow::ensure!(
            restored.contains("{ 0 }"),
            "retry workspace was not restored"
        );
        std::fs::write(&source, "pub fn value() -> u8 { 42 }\n")?;
        Ok(AgentOutcome {
            identity: identity(),
            termination: AgentTerminationReason::Completed,
            exit_code: Some(0),
            duration_ms: 1,
            usage: AgentUsage::default(),
            events: Vec::new(),
            error: None,
        })
    }
}

#[async_trait]
impl AgentExecutor for ConcurrencyAgent {
    fn identity(&self) -> &AgentIdentity {
        static IDENTITY: std::sync::OnceLock<AgentIdentity> = std::sync::OnceLock::new();
        IDENTITY.get_or_init(identity)
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        _events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        std::fs::write(
            request.workspace.join("src/lib.rs"),
            "pub fn value() -> u8 { 42 }\n",
        )?;
        Ok(AgentOutcome {
            identity: identity(),
            termination: AgentTerminationReason::Completed,
            exit_code: Some(0),
            duration_ms: 150,
            usage: AgentUsage::default(),
            events: Vec::new(),
            error: None,
        })
    }
}

struct DirectEnvironment;

#[async_trait]
impl WorkspaceEnvironment for DirectEnvironment {
    fn identity(&self) -> String {
        "host-test".into()
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

struct HiddenCheckingGrader;

#[async_trait]
impl Grader for HiddenCheckingGrader {
    fn identity(&self) -> String {
        "test-grader".into()
    }

    async fn grade(&self, request: &GradeRequest) -> Result<GraderOutcome> {
        let source = std::fs::read_to_string(request.workspace.join("src/lib.rs"))?;
        let hidden_exists = request.workspace.join("tests/hidden.rs").exists();
        let success = source.contains("42") && hidden_exists;
        Ok(GraderOutcome {
            success,
            exit_code: Some(if success { 0 } else { 1 }),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 1,
            checks: vec![GraderCheck {
                name: "hidden tests".into(),
                kind: GraderCheckKind::FailToPass,
                passed: success,
                details: String::new(),
            }],
        })
    }
}

fn config(output: &Path) -> RepositoryEngineConfig {
    RepositoryEngineConfig {
        trials: 1,
        parallelism: 1,
        output_dir: output.to_path_buf(),
        policy: ExecutionPolicyManifest {
            schema_version: 1,
            profile: "test".into(),
            agent_environment: "host-test".into(),
            verifier_environment: "test-grader".into(),
            verifier_image: None,
            network: "none".into(),
            parameters: forgetest_core::repository_report::ExecutionPolicyParameters {
                trials: 1,
                parallelism: 1,
                agent_timeout_secs: 900,
                max_agent_output_bytes: 4 * 1024 * 1024,
                max_workspace_files: 10_000,
                max_workspace_bytes: 64 * 1024 * 1024,
                max_patch_bytes: 4 * 1024 * 1024,
                ..Default::default()
            },
            digest: String::new(),
        }
        .sealed(),
        ..RepositoryEngineConfig::default()
    }
}

#[tokio::test]
async fn lifecycle_keeps_grader_hidden_and_persists_evidence() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        config(output.path()),
    );

    let report = engine
        .run(&suite, vec![Arc::new(EditingAgent { fail: false })])
        .await
        .unwrap();

    assert_eq!(report.trials.len(), 1);
    assert_eq!(report.trials[0].status, TrialStatus::Passed);
    assert!(report.trials[0].patch.contains("src/lib.rs"));
    assert_eq!(report.trials[0].changed_files.len(), 1);
    assert!(output.path().join("report.partial.json").exists());
    assert!(output
        .path()
        .join("trials")
        .read_dir()
        .unwrap()
        .next()
        .is_some());
    let stdout_artifact = report.trials[0].artifacts["grader_stdout"].clone();
    let stderr_artifact = report.trials[0].artifacts["grader_stderr"].clone();
    assert!(output.path().join(stdout_artifact).is_file());
    assert!(output.path().join(stderr_artifact).is_file());
    assert!(
        std::fs::read_to_string(fixture.path().join("tasks/fix/workspace/src/lib.rs"))
            .unwrap()
            .contains("0")
    );
}

#[tokio::test]
async fn agent_failure_is_retained_as_a_scheduled_trial() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        config(output.path()),
    );

    let report = engine
        .run(&suite, vec![Arc::new(EditingAgent { fail: true })])
        .await
        .unwrap();

    assert_eq!(report.trials.len(), 1);
    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
    assert!(report.trials[0]
        .error
        .as_deref()
        .unwrap()
        .contains("agent crashed"));
    assert_eq!(
        report.aggregate.per_agent["editing/deterministic@1#config"].scheduled,
        1
    );
}

#[tokio::test]
async fn oversized_patch_fails_instead_of_grading_partial_changes() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = config(output.path());
    engine_config.max_patch_bytes = 16;
    engine_config.policy.parameters.max_patch_bytes = 16;
    engine_config.policy = engine_config.policy.sealed();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        engine_config,
    );

    let report = engine
        .run(&suite, vec![Arc::new(EditingAgent { fail: false })])
        .await
        .unwrap();

    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
    assert_eq!(
        report.trials[0].termination_reason,
        Some(AgentTerminationReason::OutputLimit)
    );
    assert!(report.trials[0].grader.is_none());
    assert!(report.trials[0]
        .error
        .as_deref()
        .unwrap()
        .contains("patch exceeds byte limit"));
}

#[tokio::test]
async fn repository_trials_respect_parallelism_and_keep_stable_order() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = config(output.path());
    engine_config.trials = 4;
    engine_config.parallelism = 2;
    engine_config.policy.parameters.trials = 4;
    engine_config.policy.parameters.parallelism = 2;
    engine_config.policy = engine_config.policy.sealed();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        engine_config,
    );
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let agent: Arc<dyn AgentExecutor> = Arc::new(ConcurrencyAgent {
        active,
        peak: Arc::clone(&peak),
    });

    let started = Instant::now();
    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert!(started.elapsed() < Duration::from_millis(550));
    assert_eq!(
        report
            .trials
            .iter()
            .map(|trial| trial.trial_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
}

#[tokio::test]
async fn agent_retry_restores_workspace_and_records_attempts() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = config(output.path());
    engine_config.agent_limits.max_retries = 1;
    engine_config.policy.parameters.agent_retries = 1;
    engine_config.policy = engine_config.policy.sealed();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        engine_config,
    );
    let calls = Arc::new(AtomicUsize::new(0));

    let report = engine
        .run(
            &suite,
            vec![Arc::new(FlakyAgent {
                calls: Arc::clone(&calls),
            })],
        )
        .await
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(report.trials[0].status, TrialStatus::Passed);
    assert_eq!(report.trials[0].agent_attempts, 2);
    assert!(report.trials[0]
        .events
        .iter()
        .any(|event| event.message.contains("retrying agent")));
}

#[tokio::test]
async fn repository_engine_rejects_tampered_execution_policy() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = config(output.path());
    engine_config.policy.parameters.parallelism = 99;
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        engine_config,
    );

    let error = engine
        .run(&suite, vec![Arc::new(EditingAgent { fail: false })])
        .await
        .unwrap_err();

    assert!(error.to_string().contains("execution policy digest"));
}
