use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use forgetest_core::agent::{
    AgentEvent, AgentEventKind, AgentExecutor, AgentIdentity, AgentOutcome, AgentRequest,
    AgentTerminationReason, AgentUsage, CostSource, EventSink, GradeRequest, Grader,
    WorkspaceEnvironment,
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

/// Fixes the task and reports fixed usage, like a metered external agent.
struct MeteredAgent {
    calls: Arc<AtomicUsize>,
    usage: AgentUsage,
    termination: AgentTerminationReason,
    delay: Duration,
}

impl MeteredAgent {
    fn new(usage: AgentUsage) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            usage,
            termination: AgentTerminationReason::Completed,
            delay: Duration::ZERO,
        }
    }
}

#[async_trait]
impl AgentExecutor for MeteredAgent {
    fn identity(&self) -> &AgentIdentity {
        static IDENTITY: std::sync::OnceLock<AgentIdentity> = std::sync::OnceLock::new();
        IDENTITY.get_or_init(identity)
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        _events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        std::fs::write(
            request.workspace.join("src/lib.rs"),
            "pub fn value() -> u8 { 42 }\n",
        )?;
        Ok(AgentOutcome {
            identity: identity(),
            termination: self.termination,
            exit_code: Some(0),
            duration_ms: 1,
            usage: self.usage.clone(),
            events: Vec::new(),
            error: (self.termination != AgentTerminationReason::Completed)
                .then(|| "budget unverifiable: no usage".to_string()),
        })
    }
}

fn vendor_usage(cost: f64) -> AgentUsage {
    AgentUsage {
        input_tokens: 100,
        output_tokens: 10,
        estimated_cost_usd: cost,
        cost_source: Some(CostSource::Vendor),
        ..AgentUsage::default()
    }
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
    // A failed attempt's consumption is unknown, never zero.
    assert_eq!(
        report.trials[0].usage.cost_source,
        Some(CostSource::Unavailable)
    );
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

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(peak.load(Ordering::SeqCst), 2);
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

fn capped_config(
    output: &Path,
    trials: u32,
    parallelism: usize,
    cap: f64,
) -> RepositoryEngineConfig {
    let mut engine_config = config(output);
    engine_config.trials = trials;
    engine_config.parallelism = parallelism;
    engine_config.policy.parameters.trials = trials;
    engine_config.policy.parameters.parallelism = parallelism;
    engine_config.policy.parameters.max_run_cost_usd = Some(cap);
    engine_config.policy = engine_config.policy.sealed();
    engine_config
}

#[tokio::test]
async fn run_cost_breaker_cancels_unstarted_trials_once_cap_is_reached() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        capped_config(output.path(), 4, 1, 1.0),
    );
    let agent = Arc::new(MeteredAgent::new(vendor_usage(0.6)));
    let calls = Arc::clone(&agent.calls);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        report
            .trials
            .iter()
            .map(|trial| (trial.trial_index, trial.status))
            .collect::<Vec<_>>(),
        vec![
            (1, TrialStatus::Passed),
            (2, TrialStatus::Passed),
            (3, TrialStatus::Cancelled),
            (4, TrialStatus::Cancelled),
        ]
    );
    for cancelled in &report.trials[2..] {
        assert_eq!(cancelled.agent_attempts, 0);
        assert!(cancelled
            .error
            .as_deref()
            .unwrap()
            .contains("run cost cap reached"));
        assert!(cancelled.usage.cost_source.is_none());
    }
    let stats = &report.aggregate.per_agent["editing/deterministic@1#config"];
    assert_eq!(stats.scheduled, 4);
    assert_eq!(stats.cancelled, 2);
    assert!((stats.total_cost_usd - 1.2).abs() < 1e-9);
    assert_eq!(stats.cost_unavailable_trials, Some(0));
    assert_eq!(output.path().join("trials").read_dir().unwrap().count(), 4);
    assert!(report.policy.verify_digest());
}

#[tokio::test]
async fn run_cost_breaker_lets_in_flight_trials_finish() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        capped_config(output.path(), 4, 2, 1.0),
    );
    let mut agent = MeteredAgent::new(vendor_usage(0.6));
    agent.delay = Duration::from_millis(50);
    let agent = Arc::new(agent);
    let calls = Arc::clone(&agent.calls);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    // Trial 3 starts after trial 1 finishes below the cap; trial 2 then
    // reaches it, so only trial 4 is never started. The overshoot is one
    // in-flight trial, within (parallelism - 1) x the per-trial cost.
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(report.trials[3].status, TrialStatus::Cancelled);
    assert_eq!(
        report.aggregate.per_agent["editing/deterministic@1#config"].cancelled,
        1
    );
}

#[tokio::test]
async fn run_cost_breaker_trips_when_a_finished_trial_has_unavailable_cost() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        capped_config(output.path(), 3, 1, 100.0),
    );
    let agent = Arc::new(MeteredAgent::new(AgentUsage::unavailable()));
    let calls = Arc::clone(&agent.calls);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.trials[0].status, TrialStatus::Passed);
    for cancelled in &report.trials[1..] {
        assert_eq!(cancelled.status, TrialStatus::Cancelled);
        assert_eq!(cancelled.agent_attempts, 0);
        assert!(cancelled
            .error
            .as_deref()
            .unwrap()
            .contains("run cost unverifiable"));
    }
    let stats = &report.aggregate.per_agent["editing/deterministic@1#config"];
    assert_eq!(stats.scheduled, 3);
    assert_eq!(stats.cost_unavailable_trials, Some(1));
}

#[tokio::test]
async fn unavailable_cost_does_not_stop_a_run_without_a_run_cap() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = capped_config(output.path(), 3, 1, 1.0);
    engine_config.policy.parameters.max_run_cost_usd = None;
    engine_config.policy = engine_config.policy.sealed();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        engine_config,
    );
    let agent = Arc::new(MeteredAgent::new(AgentUsage::unavailable()));
    let calls = Arc::clone(&agent.calls);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(report
        .trials
        .iter()
        .all(|trial| trial.status == TrialStatus::Passed));
}

#[tokio::test]
async fn unverifiable_budget_is_an_agent_error_and_failure_paths_keep_usage() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();

    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        config(output.path()),
    );
    let mut unverifiable = MeteredAgent::new(AgentUsage::unavailable());
    unverifiable.termination = AgentTerminationReason::BudgetUnverifiable;
    let report = engine
        .run(&suite, vec![Arc::new(unverifiable)])
        .await
        .unwrap();
    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
    assert_eq!(
        report.trials[0].termination_reason,
        Some(AgentTerminationReason::BudgetUnverifiable)
    );
    assert!(report.trials[0].grader.is_none());

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
        .run(
            &suite,
            vec![Arc::new(MeteredAgent::new(vendor_usage(0.25)))],
        )
        .await
        .unwrap();
    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
    assert_eq!(report.trials[0].usage.cost_source, Some(CostSource::Vendor));
    assert_eq!(report.trials[0].usage.estimated_cost_usd, 0.25);
    assert_eq!(report.trials[0].usage.input_tokens, 100);
}

#[tokio::test]
async fn benchmark_retry_stops_after_an_attempt_with_unknown_usage() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut engine_config = config(output.path());
    engine_config.agent_limits.max_retries = 1;
    engine_config.agent_limits.max_tokens = Some(1_000);
    engine_config.agent_limits.require_enforceable_usage = true;
    engine_config.policy.parameters.agent_retries = 1;
    engine_config.policy.parameters.max_agent_tokens = Some(1_000);
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

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
    assert_eq!(
        report.trials[0].termination_reason,
        Some(AgentTerminationReason::BudgetUnverifiable)
    );
    assert_eq!(
        report.trials[0].usage.cost_source,
        Some(CostSource::Unavailable)
    );
    let error = report.trials[0].error.as_deref().unwrap();
    assert!(error.contains("budget unverifiable"));
    assert!(error.contains("transient agent failure"));
    assert!(!report.trials[0]
        .events
        .iter()
        .any(|event| event.message.contains("retrying agent")));
}

/// Returns a configured outcome per call (the last one repeats) and records
/// which agent ran and the limits each attempt received.
struct AttemptAgent {
    identity: AgentIdentity,
    outcomes: Vec<(AgentTerminationReason, AgentUsage)>,
    calls: Arc<AtomicUsize>,
    started: Arc<std::sync::Mutex<Vec<String>>>,
    limits: Arc<std::sync::Mutex<Vec<forgetest_core::agent::AgentLimits>>>,
}

impl AttemptAgent {
    fn new(adapter: &str, outcomes: Vec<(AgentTerminationReason, AgentUsage)>) -> Self {
        Self {
            identity: AgentIdentity {
                adapter: adapter.into(),
                ..identity()
            },
            outcomes,
            calls: Arc::new(AtomicUsize::new(0)),
            started: Arc::new(std::sync::Mutex::new(Vec::new())),
            limits: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl AgentExecutor for AttemptAgent {
    fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    async fn execute(
        &self,
        request: &AgentRequest,
        _events: &dyn EventSink,
    ) -> Result<AgentOutcome> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.started
            .lock()
            .unwrap()
            .push(self.identity.adapter.clone());
        self.limits.lock().unwrap().push(request.limits.clone());
        let (termination, usage) = self.outcomes[call.min(self.outcomes.len() - 1)].clone();
        std::fs::write(
            request.workspace.join("src/lib.rs"),
            "pub fn value() -> u8 { 42 }\n",
        )?;
        Ok(AgentOutcome {
            identity: self.identity.clone(),
            termination,
            exit_code: Some(i32::from(termination != AgentTerminationReason::Completed)),
            duration_ms: 1,
            usage,
            events: Vec::new(),
            error: (termination != AgentTerminationReason::Completed)
                .then(|| "agent exited non-zero".to_string()),
        })
    }
}

fn retry_config(
    output: &Path,
    configure: impl FnOnce(&mut RepositoryEngineConfig),
) -> RepositoryEngineConfig {
    let mut engine_config = config(output);
    engine_config.agent_limits.max_retries = 1;
    engine_config.policy.parameters.agent_retries = 1;
    configure(&mut engine_config);
    engine_config.policy = engine_config.policy.sealed();
    engine_config
}

#[tokio::test]
async fn run_cost_breaker_treats_unmetered_or_non_finite_cost_as_unverifiable() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    for usage in [
        AgentUsage::default(),
        vendor_usage(f64::NAN),
        vendor_usage(f64::INFINITY),
        vendor_usage(-1.0),
    ] {
        let output = tempfile::tempdir().unwrap();
        let engine = RepositoryEngine::new(
            Arc::new(DirectEnvironment),
            Arc::new(HiddenCheckingGrader),
            capped_config(output.path(), 3, 1, 100.0),
        );
        let agent = Arc::new(MeteredAgent::new(usage.clone()));
        let calls = Arc::clone(&agent.calls);

        let report = engine.run(&suite, vec![agent]).await.unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1, "{usage:?}");
        for cancelled in &report.trials[1..] {
            assert_eq!(cancelled.status, TrialStatus::Cancelled);
            assert!(cancelled
                .error
                .as_deref()
                .unwrap()
                .contains("run cost unverifiable"));
        }
    }
}

#[tokio::test]
async fn engine_rejects_an_invalid_run_cap_before_any_trial() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    for cap in [0.0, -1.0] {
        let output = tempfile::tempdir().unwrap();
        let engine = RepositoryEngine::new(
            Arc::new(DirectEnvironment),
            Arc::new(HiddenCheckingGrader),
            capped_config(output.path(), 1, 1, cap),
        );
        let agent = Arc::new(MeteredAgent::new(vendor_usage(0.1)));
        let calls = Arc::clone(&agent.calls);

        let error = engine.run(&suite, vec![agent]).await.unwrap_err();

        assert!(error.to_string().contains("run cost cap"), "{error:#}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!output.path().join("trials").exists());
    }
}

#[tokio::test]
async fn benchmark_run_cap_alone_fails_closed_on_retry_with_unknown_cost() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        retry_config(output.path(), |config| {
            config.agent_limits.require_enforceable_usage = true;
            config.policy.parameters.max_run_cost_usd = Some(10.0);
        }),
    );
    let unpriced = AgentUsage {
        input_tokens: 100,
        output_tokens: 10,
        ..AgentUsage::unavailable()
    };
    let agent = Arc::new(AttemptAgent::new(
        "editing",
        vec![(AgentTerminationReason::ExitNonZero, unpriced)],
    ));
    let calls = Arc::clone(&agent.calls);
    let limits = Arc::clone(&agent.limits);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(limits.lock().unwrap()[0].run_cost_capped);
    assert_eq!(
        report.trials[0].termination_reason,
        Some(AgentTerminationReason::BudgetUnverifiable)
    );
    assert_eq!(report.trials[0].status, TrialStatus::AgentError);
}

#[tokio::test]
async fn development_cost_cap_stops_retry_instead_of_treating_unknown_cost_as_zero() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        retry_config(output.path(), |config| {
            config.agent_limits.max_cost_usd = Some(1.0);
            config.policy.parameters.max_agent_cost_usd = Some(1.0);
        }),
    );
    let agent = Arc::new(AttemptAgent::new(
        "editing",
        vec![(
            AgentTerminationReason::ExitNonZero,
            AgentUsage {
                input_tokens: 100,
                output_tokens: 10,
                ..AgentUsage::unavailable()
            },
        )],
    ));
    let calls = Arc::clone(&agent.calls);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        report.trials[0].termination_reason,
        Some(AgentTerminationReason::BudgetUnverifiable)
    );
    assert!(report.trials[0]
        .error
        .as_deref()
        .unwrap()
        .contains("budget unverifiable"));
}

#[tokio::test]
async fn retry_token_budget_subtracts_cache_reads_and_writes() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        retry_config(output.path(), |config| {
            config.agent_limits.max_tokens = Some(1_000);
            config.policy.parameters.max_agent_tokens = Some(1_000);
        }),
    );
    let first = AgentUsage {
        input_tokens: 100,
        cached_tokens: 50,
        cache_write_tokens: 25,
        output_tokens: 10,
        ..vendor_usage(0.01)
    };
    let agent = Arc::new(AttemptAgent::new(
        "editing",
        vec![
            (AgentTerminationReason::ExitNonZero, first),
            (AgentTerminationReason::Completed, vendor_usage(0.01)),
        ],
    ));
    let limits = Arc::clone(&agent.limits);

    let report = engine.run(&suite, vec![agent]).await.unwrap();

    let limits = limits.lock().unwrap();
    assert_eq!(limits.len(), 2);
    assert_eq!(limits[0].max_tokens, Some(1_000));
    assert_eq!(limits[1].max_tokens, Some(1_000 - 185));
    assert_eq!(report.trials[0].status, TrialStatus::Passed);
    assert_eq!(report.trials[0].agent_attempts, 2);
    assert_eq!(report.trials[0].usage.cache_write_tokens, 25);
}

#[tokio::test]
async fn round_robin_dispatch_balances_agents_and_keeps_report_order() {
    let fixture = tempfile::tempdir().unwrap();
    write_suite(fixture.path());
    let suite = load_suite(&fixture.path().join("suite.toml")).unwrap();
    let output = tempfile::tempdir().unwrap();
    let engine = RepositoryEngine::new(
        Arc::new(DirectEnvironment),
        Arc::new(HiddenCheckingGrader),
        capped_config(output.path(), 3, 1, 1.0),
    );
    let started = Arc::new(std::sync::Mutex::new(Vec::new()));
    let agents: Vec<Arc<dyn AgentExecutor>> = ["alpha", "beta"]
        .into_iter()
        .map(|adapter| {
            let mut agent = AttemptAgent::new(
                adapter,
                vec![(AgentTerminationReason::Completed, vendor_usage(0.3))],
            );
            agent.started = Arc::clone(&started);
            Arc::new(agent) as Arc<dyn AgentExecutor>
        })
        .collect();

    let report = engine.run(&suite, agents).await.unwrap();

    // 0.3 + 0.3 + 0.3 + 0.3 reaches the cap after four starts, so each
    // agent ran trials 1 and 2 and only trial 3 was cancelled for both.
    assert_eq!(
        *started.lock().unwrap(),
        vec!["alpha", "beta", "alpha", "beta"]
    );
    assert_eq!(
        report
            .trials
            .iter()
            .map(|trial| (
                trial.agent.adapter.as_str(),
                trial.trial_index,
                trial.status
            ))
            .collect::<Vec<_>>(),
        vec![
            ("alpha", 1, TrialStatus::Passed),
            ("alpha", 2, TrialStatus::Passed),
            ("alpha", 3, TrialStatus::Cancelled),
            ("beta", 1, TrialStatus::Passed),
            ("beta", 2, TrialStatus::Passed),
            ("beta", 3, TrialStatus::Cancelled),
        ]
    );
}
