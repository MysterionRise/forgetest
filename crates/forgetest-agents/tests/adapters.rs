use forgetest_agents::{
    builtin_profile, docker_preflight_arguments, doctor, ensure_caps_enforceable,
    parse_container_doctor_output, profile_configuration_digest, CommandProfile,
    DirectWorkspaceEnvironment, DockerAgentConfig, DockerProcessAgent, EventParser,
    MemoryEventSink, ProcessAgent, ScriptedAgent, ScriptedEdit,
};
use forgetest_core::agent::{
    AgentExecutor, AgentIdentity, AgentLimits, AgentOutcome, AgentRequest, AgentTerminationReason,
    CostSource, WorkspaceEnvironment,
};
use forgetest_core::pricing::{ModelPricing, PricingTable};
use uuid::Uuid;

fn identity(adapter: &str) -> AgentIdentity {
    AgentIdentity {
        adapter: adapter.into(),
        adapter_version: "test".into(),
        executable_sha256: None,
        model: "test-model".into(),
        configuration_digest: "config".into(),
    }
}

#[tokio::test]
async fn scripted_agent_applies_safe_edits_and_emits_events() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/lib.rs"),
        "pub fn value() -> u8 { 0 }\n",
    )
    .unwrap();
    let agent = ScriptedAgent::new(
        identity("scripted"),
        vec![ScriptedEdit {
            path: "src/lib.rs".into(),
            content: "pub fn value() -> u8 { 42 }\n".into(),
        }],
    );
    let sink = MemoryEventSink::default();

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "Fix value".into(),
                workspace: workspace.path().to_path_buf(),
                limits: AgentLimits::default(),
            },
            &sink,
        )
        .await
        .unwrap();

    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
    assert!(std::fs::read_to_string(workspace.path().join("src/lib.rs"))
        .unwrap()
        .contains("42"));
    assert!(sink.events().len() >= 2);
}

#[tokio::test]
async fn scripted_agent_rejects_path_traversal() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = ScriptedAgent::new(
        identity("scripted"),
        vec![ScriptedEdit {
            path: "../outside".into(),
            content: "no".into(),
        }],
    );

    let error = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: String::new(),
                workspace: workspace.path().to_path_buf(),
                limits: AgentLimits::default(),
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("relative"));
}

#[tokio::test]
async fn direct_environment_delegates_to_agent() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = ScriptedAgent::new(identity("scripted"), Vec::new());
    let environment = DirectWorkspaceEnvironment;
    let outcome = environment
        .execute(
            &agent,
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: String::new(),
                workspace: workspace.path().to_path_buf(),
                limits: AgentLimits::default(),
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(environment.identity(), "host-trusted");
    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
}

#[test]
fn built_in_profiles_are_noninteractive_and_do_not_embed_prompts() {
    let codex = CommandProfile::codex("gpt-test");
    assert_eq!(codex.executable, "codex");
    assert!(codex
        .arguments
        .windows(2)
        .any(|args| args == ["--model", "gpt-test"]));
    assert!(codex.arguments.iter().any(|arg| arg == "--json"));
    assert!(codex.arguments.iter().any(|arg| arg == "--ephemeral"));
    assert!(codex
        .arguments
        .windows(2)
        .any(|args| args == ["--sandbox", "workspace-write"]));
    assert_eq!(codex.parser, EventParser::CodexJsonl);
    assert!(!codex.arguments.iter().any(|arg| arg.contains("PROMPT")));

    let claude = CommandProfile::claude("claude-test");
    assert_eq!(claude.executable, "claude");
    assert!(claude.arguments.iter().any(|arg| arg == "--print"));
    assert!(claude
        .arguments
        .windows(2)
        .any(|args| args == ["--output-format", "stream-json"]));
    assert!(claude
        .arguments
        .iter()
        .any(|arg| arg == "--no-session-persistence"));
    assert!(claude
        .arguments
        .windows(2)
        .any(|args| { args == ["--permission-mode", "bypassPermissions"] }));
    assert_eq!(claude.parser, EventParser::ClaudeStreamJson);
}

#[test]
fn builtin_profiles_apply_effort_and_have_content_identity() {
    let codex = builtin_profile("codex", "gpt-test", Some("high")).unwrap();
    assert!(codex
        .arguments
        .windows(2)
        .any(|pair| pair == ["-c", "model_reasoning_effort=\"high\""]));
    let first = profile_configuration_digest(
        &codex,
        "agent@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    let repeated = profile_configuration_digest(
        &codex,
        "agent@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    let changed = profile_configuration_digest(
        &codex,
        "agent@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    assert_eq!(first, repeated);
    assert_eq!(first.len(), 64);
    assert_ne!(first, changed);
    assert!(builtin_profile("unknown", "model", None)
        .unwrap_err()
        .to_string()
        .contains("unsupported"));
}

#[test]
fn docker_agent_args_isolate_host_state_and_record_network() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = DockerProcessAgent::new(
        CommandProfile::codex("gpt-test"),
        identity("codex"),
        DockerAgentConfig {
            image: "codex-agent@sha256:abc".into(),
            ..DockerAgentConfig::default()
        },
    );
    let args = agent.docker_arguments("trial-name", workspace.path());

    assert!(args.windows(2).any(|pair| pair == ["--name", "trial-name"]));
    assert!(args.iter().any(|argument| argument == "--interactive"));
    assert!(args.windows(2).any(|pair| pair == ["--network", "bridge"]));
    assert!(args.windows(2).any(|pair| pair == ["--cap-drop", "ALL"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--security-opt", "no-new-privileges"]));
    assert!(args.iter().any(|argument| argument == "--read-only"));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--env", "OPENAI_API_KEY"]));
    assert!(!args.iter().any(|argument| argument.contains("sk-")));
    assert!(!args.iter().any(|argument| argument.contains("docker.sock")));
    assert!(args.iter().any(|argument| argument == "/work"));
    assert!(args
        .iter()
        .any(|argument| argument == "codex-agent@sha256:abc"));
    assert!(args
        .windows(2)
        .filter(|pair| pair[0] == "--mount")
        .all(|pair| !pair[1].ends_with(",rw")));
}

#[tokio::test]
async fn docker_agent_forwards_prompt_when_enabled() {
    if std::env::var("FORGETEST_DOCKER_TEST").as_deref() != Ok("1") {
        return;
    }

    let workspace = tempfile::tempdir().unwrap();
    let profile = CommandProfile {
        name: "stdin-probe".into(),
        executable: "sh".into(),
        arguments: vec![
            "-c".into(),
            "IFS= read -r line || true; printf 'received:%s\\n' \"$line\"".into(),
        ],
        version_arguments: vec!["--version".into()],
        parser: EventParser::Text,
        environment_allowlist: Vec::new(),
    };
    let agent = DockerProcessAgent::new(
        profile,
        identity("stdin-probe"),
        DockerAgentConfig {
            image: "forgetest-runner-rust:0.1.0".into(),
            network: "none".into(),
            ..DockerAgentConfig::default()
        },
    );

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "stdin-probe".into(),
                prompt: "benchmark prompt".into(),
                workspace: workspace.path().to_path_buf(),
                limits: AgentLimits::default(),
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        outcome.termination,
        AgentTerminationReason::Completed,
        "Docker agent did not complete: {outcome:#?}"
    );
    assert!(outcome
        .events
        .iter()
        .any(|event| event.message == "received:benchmark prompt"));
}

#[test]
fn docker_preflight_is_network_disabled_and_has_no_credentials() {
    let profile = CommandProfile::codex("gpt-test");

    let args = docker_preflight_arguments(&profile, "codex-agent@sha256:abc");

    assert!(args.windows(2).any(|pair| pair == ["--network", "none"]));
    assert!(args.iter().any(|argument| argument == "--read-only"));
    assert!(args.windows(2).any(|pair| pair == ["--cap-drop", "ALL"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--security-opt", "no-new-privileges"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--user", "65532:65532"]));
    assert!(!args.iter().any(|argument| argument == "--env"));
    assert!(!args.iter().any(|argument| argument == "OPENAI_API_KEY"));
    assert!(args.iter().any(|argument| argument == "codex"));
    assert!(args.iter().any(|argument| argument == "--version"));
}

#[test]
fn container_preflight_output_is_parsed_strictly() {
    let report = parse_container_doctor_output(
        b"path=/usr/local/bin/codex\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  /usr/local/bin/codex\ncodex-cli 1.2.3\n",
    )
    .unwrap();

    assert_eq!(report.executable_path, "/usr/local/bin/codex");
    assert_eq!(report.version, "codex-cli 1.2.3");
    assert_eq!(report.executable_sha256.len(), 64);

    assert!(
        parse_container_doctor_output(b"path=/bin/x\nnot-a-hash /bin/x\nv1\n")
            .unwrap_err()
            .to_string()
            .contains("SHA-256")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_enforces_output_limit() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("noisy.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nwhile true; do printf '012345678901234567890123456789\\n'; done\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let mut profile = CommandProfile::generic("/bin/sh", EventParser::Text);
    profile
        .arguments
        .push(script.to_string_lossy().into_owned());
    let agent = ProcessAgent::new(profile, identity("generic"));

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "work".into(),
                workspace: root.path().to_path_buf(),
                limits: AgentLimits {
                    timeout_secs: 10,
                    max_output_bytes: 256,
                    ..AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.termination, AgentTerminationReason::OutputLimit);
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_rejects_fast_output_burst() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("burst.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 64 ]; do printf '0123456789abcdef'; i=$((i + 1)); done\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let mut profile = CommandProfile::generic("/bin/sh", EventParser::Text);
    profile
        .arguments
        .push(script.to_string_lossy().into_owned());
    let agent = ProcessAgent::new(profile, identity("generic"));

    for _ in 0..16 {
        let outcome = agent
            .execute(
                &AgentRequest {
                    trial_id: Uuid::new_v4(),
                    task_id: "task".into(),
                    prompt: "work".into(),
                    workspace: root.path().to_path_buf(),
                    limits: AgentLimits {
                        timeout_secs: 10,
                        max_output_bytes: 32,
                        ..AgentLimits::default()
                    },
                },
                &MemoryEventSink::default(),
            )
            .await
            .unwrap();

        assert_eq!(outcome.termination, AgentTerminationReason::OutputLimit);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_closes_prompt_before_joining_descendant_output() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("inherited-pipes.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "exec 3<&0\n",
            "(sleep 0.1; IFS= read -r _ <&3 || true) &\n",
            "exec 3<&-\n",
            "exit 0\n"
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);
    let agent = ProcessAgent::new(profile, identity("generic"));

    let request = AgentRequest {
        trial_id: Uuid::new_v4(),
        task_id: "task".into(),
        prompt: "x".repeat(8 * 1024 * 1024),
        workspace: root.path().to_path_buf(),
        limits: AgentLimits {
            timeout_secs: 5,
            max_output_bytes: 1024,
            ..AgentLimits::default()
        },
    };
    let events = MemoryEventSink::default();
    let execution = agent.execute(&request, &events);
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(2), execution)
        .await
        .expect("agent hung while a descendant retained inherited pipes")
        .unwrap();

    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_limits_normalized_event_count() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("many-events.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "i=0\n",
            "while [ \"$i\" -lt 10001 ]; do printf 'x\\n'; i=$((i + 1)); done\n",
            "printf 'warning\\n' >&2\n"
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);
    let agent = ProcessAgent::new(profile, identity("generic"));

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "work".into(),
                workspace: root.path().to_path_buf(),
                limits: AgentLimits {
                    timeout_secs: 10,
                    max_output_bytes: 64 * 1024,
                    ..AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.termination, AgentTerminationReason::OutputLimit);
    assert_eq!(outcome.events.len(), 10_000);
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_classifies_reported_usage_over_budget() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("usage.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' ",
            "'{\"type\":\"usage\",\"usage\":{\"input_tokens\":8,",
            "\"output_tokens\":5,\"estimated_cost_usd\":1.25}}'\n"
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let agent = ProcessAgent::new(
        CommandProfile::generic(script.to_string_lossy(), EventParser::GenericJsonl),
        identity("generic"),
    );

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "work".into(),
                workspace: root.path().to_path_buf(),
                limits: AgentLimits {
                    max_tokens: Some(10),
                    max_cost_usd: Some(1.0),
                    ..AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.termination, AgentTerminationReason::BudgetExceeded);
    assert_eq!(outcome.usage.input_tokens, 8);
    assert_eq!(outcome.usage.output_tokens, 5);
    assert_eq!(outcome.usage.estimated_cost_usd, 1.25);
    assert_eq!(outcome.usage.cost_source, Some(CostSource::Vendor));
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_timeout_terminates_descendants() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("descendant-survived");
    let script = root.path().join("descendant.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n(sleep 2; printf survived > \"$1\") &\nwhile true; do sleep 1; done\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let mut profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);
    profile
        .arguments
        .push(marker.to_string_lossy().into_owned());
    let agent = ProcessAgent::new(profile, identity("generic"));

    let outcome = agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "work".into(),
                workspace: root.path().to_path_buf(),
                limits: AgentLimits {
                    timeout_secs: 1,
                    ..AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    assert_eq!(outcome.termination, AgentTerminationReason::Timeout);
    assert!(
        !marker.exists(),
        "background descendant survived the process-group termination"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn process_agent_streams_events_before_process_exit() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct SignalSink {
        seen: AtomicBool,
        notify: tokio::sync::Notify,
    }

    impl forgetest_core::agent::EventSink for SignalSink {
        fn emit(&self, _: &forgetest_core::agent::AgentEvent) -> anyhow::Result<()> {
            self.seen.store(true, Ordering::SeqCst);
            self.notify.notify_one();
            Ok(())
        }
    }

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("streaming.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'working\\n'\nwhile [ ! -f \"$1\" ]; do sleep 0.01; done\nprintf 'done\\n'\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let marker = root.path().join("continue");
    let mut profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);
    profile.arguments.push("{workspace}/continue".to_string());
    let agent = Arc::new(ProcessAgent::new(profile, identity("generic")));
    let sink = Arc::new(SignalSink {
        seen: AtomicBool::new(false),
        notify: tokio::sync::Notify::new(),
    });
    let request = AgentRequest {
        trial_id: Uuid::new_v4(),
        task_id: "task".into(),
        prompt: "work".into(),
        workspace: root.path().to_path_buf(),
        limits: AgentLimits::default(),
    };
    let running = {
        let agent = Arc::clone(&agent);
        let sink = Arc::clone(&sink);
        tokio::spawn(async move { agent.execute(&request, sink.as_ref()).await })
    };

    tokio::time::timeout(std::time::Duration::from_secs(10), sink.notify.notified())
        .await
        .expect("first event was buffered until process exit");
    assert!(
        sink.seen.load(Ordering::SeqCst),
        "first event was buffered until process exit"
    );
    std::fs::write(marker, "continue").unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), running)
            .await
            .expect("agent did not exit after streaming handshake")
            .unwrap()
            .unwrap()
            .termination,
        AgentTerminationReason::Completed
    );
}

#[cfg(unix)]
#[test]
fn doctor_reports_version_without_exposing_auth_values() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("fake-agent");
    std::fs::write(&script, "#!/bin/sh\necho 'fake-agent 1.2.3'\n").unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);

    let result = doctor(&profile).unwrap();

    assert!(result.executable_found);
    assert_eq!(result.version.as_deref(), Some("fake-agent 1.2.3"));
    assert!(!format!("{result:?}").contains("sk-"));
}

fn pricing_for(model: &str) -> PricingTable {
    PricingTable {
        schema_version: 1,
        retrieved_on: "2026-10-15".into(),
        models: vec![ModelPricing {
            model: model.into(),
            input_usd_per_mtok: 2.0,
            cache_read_usd_per_mtok: 0.5,
            cache_write_usd_per_mtok: 4.0,
            output_usd_per_mtok: 10.0,
        }],
    }
}

/// Run a shell script that prints `lines` through `parser`, as a vendor CLI would.
#[cfg(unix)]
async fn run_stream(
    parser: EventParser,
    lines: &[&str],
    pricing: Option<PricingTable>,
    limits: AgentLimits,
) -> AgentOutcome {
    let mut body = String::from("cat >/dev/null\n");
    for line in lines {
        body.push_str(&format!("printf '%s\\n' '{line}'\n"));
    }
    run_script(parser, &body, pricing, limits).await
}

/// Run an arbitrary shell script body as the agent.
#[cfg(unix)]
async fn run_script(
    parser: EventParser,
    body: &str,
    pricing: Option<PricingTable>,
    limits: AgentLimits,
) -> AgentOutcome {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("vendor-stream.sh");
    std::fs::write(&script, body).unwrap();
    let mut profile = CommandProfile::generic("/bin/sh", parser);
    profile
        .arguments
        .push(script.to_string_lossy().into_owned());
    ProcessAgent::new(profile, identity("vendor"))
        .with_pricing(pricing)
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "task".into(),
                prompt: "work".into(),
                workspace: root.path().to_path_buf(),
                limits,
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap()
}

const CLAUDE_STREAM: &[&str] = &[
    r#"{"type":"system","subtype":"init","model":"test-model"}"#,
    r#"{"type":"assistant","message":{"usage":{"input_tokens":3,"output_tokens":1}}}"#,
    r#"{"type":"result","subtype":"success","total_cost_usd":0.0123,"usage":{"input_tokens":10,"cache_creation_input_tokens":50,"cache_read_input_tokens":300,"output_tokens":20}}"#,
];

const CODEX_STREAM: &[&str] = &[
    r#"{"type":"thread.started","thread_id":"t"}"#,
    r#"{"type":"turn.started"}"#,
    r#"{"type":"item.completed","item":{"type":"agent_message","text":"done"}}"#,
    r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":30}}"#,
];

#[cfg(unix)]
#[tokio::test]
async fn claude_result_usage_is_normalized_with_vendor_cost_first() {
    let outcome = run_stream(
        EventParser::ClaudeStreamJson,
        CLAUDE_STREAM,
        Some(pricing_for("test-model")),
        AgentLimits::default(),
    )
    .await;

    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
    assert_eq!(outcome.usage.input_tokens, 10);
    assert_eq!(outcome.usage.cached_tokens, 300);
    assert_eq!(outcome.usage.cache_write_tokens, 50);
    assert_eq!(outcome.usage.output_tokens, 20);
    assert_eq!(outcome.usage.estimated_cost_usd, 0.0123);
    assert_eq!(outcome.usage.cost_source, Some(CostSource::Vendor));
}

#[cfg(unix)]
#[tokio::test]
async fn codex_turn_usage_is_priced_from_the_exact_model_entry() {
    let outcome = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        Some(pricing_for("test-model")),
        AgentLimits::default(),
    )
    .await;

    assert_eq!(outcome.usage.input_tokens, 600);
    assert_eq!(outcome.usage.cached_tokens, 400);
    assert_eq!(outcome.usage.output_tokens, 100);
    assert_eq!(outcome.usage.reasoning_tokens, 30);
    assert_eq!(outcome.usage.cost_source, Some(CostSource::PricingTable));
    let expected = (600.0 * 2.0 + 400.0 * 0.5 + 100.0 * 10.0) / 1_000_000.0;
    assert!((outcome.usage.estimated_cost_usd - expected).abs() < 1e-12);

    let unpriced = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        Some(pricing_for("test-model-2")),
        AgentLimits::default(),
    )
    .await;
    assert_eq!(unpriced.termination, AgentTerminationReason::Completed);
    assert_eq!(unpriced.usage.input_tokens, 600);
    assert_eq!(unpriced.usage.estimated_cost_usd, 0.0);
    assert_eq!(unpriced.usage.cost_source, Some(CostSource::Unavailable));
}

#[cfg(unix)]
#[tokio::test]
async fn token_cap_counts_uncached_cached_and_output_tokens() {
    let within = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        None,
        AgentLimits {
            max_tokens: Some(1_100),
            require_enforceable_usage: true,
            ..AgentLimits::default()
        },
    )
    .await;
    assert_eq!(within.termination, AgentTerminationReason::Completed);

    let over = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        None,
        AgentLimits {
            max_tokens: Some(1_099),
            ..AgentLimits::default()
        },
    )
    .await;
    assert_eq!(over.termination, AgentTerminationReason::BudgetExceeded);
}

#[cfg(unix)]
#[tokio::test]
async fn missing_usage_is_unavailable_and_fails_closed_only_when_required() {
    let stream = &[r#"{"type":"item.completed","item":{"type":"agent_message"}}"#];
    for parser in [EventParser::CodexJsonl, EventParser::ClaudeStreamJson] {
        let development = run_stream(
            parser,
            stream,
            None,
            AgentLimits {
                max_tokens: Some(10),
                max_cost_usd: Some(1.0),
                ..AgentLimits::default()
            },
        )
        .await;
        assert_eq!(development.termination, AgentTerminationReason::Completed);
        assert_eq!(development.usage.cost_source, Some(CostSource::Unavailable));

        let benchmark = run_stream(
            parser,
            stream,
            None,
            AgentLimits {
                max_tokens: Some(10),
                require_enforceable_usage: true,
                ..AgentLimits::default()
            },
        )
        .await;
        assert_eq!(
            benchmark.termination,
            AgentTerminationReason::BudgetUnverifiable
        );
        assert!(benchmark
            .error
            .as_deref()
            .unwrap()
            .contains("budget unverifiable"));
    }

    let unpriced_cost_cap = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        None,
        AgentLimits {
            max_cost_usd: Some(1.0),
            require_enforceable_usage: true,
            ..AgentLimits::default()
        },
    )
    .await;
    assert_eq!(
        unpriced_cost_cap.termination,
        AgentTerminationReason::BudgetUnverifiable
    );

    let uncapped = run_stream(
        EventParser::CodexJsonl,
        stream,
        None,
        AgentLimits {
            require_enforceable_usage: true,
            ..AgentLimits::default()
        },
    )
    .await;
    assert_eq!(uncapped.termination, AgentTerminationReason::Completed);
    assert_eq!(uncapped.usage.cost_source, Some(CostSource::Unavailable));
}

#[test]
fn caps_are_enforceable_only_with_reported_usage_or_exact_pricing() {
    let cost_cap = AgentLimits {
        max_cost_usd: Some(1.0),
        ..AgentLimits::default()
    };
    let token_cap = AgentLimits {
        max_tokens: Some(1_000),
        ..AgentLimits::default()
    };
    let no_cap = AgentLimits::default();
    let priced = pricing_for("gpt-test");

    let check = |adapter, model, limits, run_cap, pricing| {
        ensure_caps_enforceable(adapter, model, limits, run_cap, pricing)
    };

    let error = check("codex", Some("gpt-test"), &cost_cap, None, None).unwrap_err();
    assert!(error.to_string().contains("'codex'"));
    assert!(error.to_string().contains("'gpt-test'"));
    assert!(check("codex", Some("gpt-test"), &no_cap, Some(5.0), None).is_err());
    assert!(check("codex", Some("gpt-test-2"), &cost_cap, None, Some(&priced)).is_err());
    assert!(check(
        "codex",
        Some("gpt-test"),
        &cost_cap,
        Some(5.0),
        Some(&priced)
    )
    .is_ok());
    assert!(check("codex", Some("gpt-test"), &token_cap, None, None).is_ok());
    assert!(check("claude", Some("claude-test"), &cost_cap, Some(5.0), None).is_ok());

    // Without an explicit model nothing can match an exact price.
    let error = check("codex", None, &cost_cap, None, Some(&priced)).unwrap_err();
    assert!(error.to_string().contains("without an explicit model"));
    assert!(check("claude", None, &cost_cap, None, None).is_ok());

    for limits in [&cost_cap, &token_cap] {
        let error = check(
            "scripted",
            Some("deterministic-repository-demo"),
            limits,
            None,
            Some(&priced),
        )
        .unwrap_err();
        assert!(error.to_string().contains("'scripted'"));
    }
    assert!(check("scripted", Some("demo"), &no_cap, Some(1.0), None).is_err());
    assert!(check("scripted", Some("demo"), &no_cap, None, None).is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn capped_agent_without_usage_is_unverifiable_on_exit_but_keeps_timeout() {
    let capped = AgentLimits {
        timeout_secs: 1,
        max_tokens: Some(1_000),
        require_enforceable_usage: true,
        ..AgentLimits::default()
    };

    let failed = run_script(
        EventParser::CodexJsonl,
        "cat >/dev/null\nprintf 'authentication failed\\n' >&2\nexit 3\n",
        None,
        capped.clone(),
    )
    .await;
    assert_eq!(
        failed.termination,
        AgentTerminationReason::BudgetUnverifiable
    );
    assert_eq!(failed.exit_code, Some(3));
    let error = failed.error.as_deref().unwrap();
    assert!(error.contains("budget unverifiable"));
    assert!(error.ends_with("agent also exited non-zero: authentication failed"));

    let timed_out = run_script(
        EventParser::CodexJsonl,
        "cat >/dev/null\nwhile true; do sleep 1; done\n",
        None,
        capped,
    )
    .await;
    assert_eq!(timed_out.termination, AgentTerminationReason::Timeout);
    assert_eq!(timed_out.usage.cost_source, Some(CostSource::Unavailable));
}

#[cfg(unix)]
#[tokio::test]
async fn claude_token_cap_counts_cache_write_tokens() {
    // 10 uncached + 300 cache read + 50 cache write + 20 output = 380.
    let limits = |cap| AgentLimits {
        max_tokens: Some(cap),
        require_enforceable_usage: true,
        ..AgentLimits::default()
    };
    let within = run_stream(
        EventParser::ClaudeStreamJson,
        CLAUDE_STREAM,
        None,
        limits(380),
    )
    .await;
    assert_eq!(within.termination, AgentTerminationReason::Completed);
    let over = run_stream(
        EventParser::ClaudeStreamJson,
        CLAUDE_STREAM,
        None,
        limits(340),
    )
    .await;
    assert_eq!(over.termination, AgentTerminationReason::BudgetExceeded);
}

#[cfg(unix)]
#[tokio::test]
async fn codex_last_turn_wins_and_claude_without_result_is_unavailable() {
    let codex = run_stream(
        EventParser::CodexJsonl,
        &[
            r#"{"type":"turn.completed","usage":{"input_tokens":9000,"cached_input_tokens":0,"output_tokens":900}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":50,"cached_input_tokens":20,"output_tokens":5}}"#,
        ],
        Some(pricing_for("test-model")),
        AgentLimits::default(),
    )
    .await;
    assert_eq!(codex.usage.input_tokens, 30);
    assert_eq!(codex.usage.cached_tokens, 20);
    assert_eq!(codex.usage.output_tokens, 5);
    assert_eq!(codex.usage.cost_source, Some(CostSource::PricingTable));

    let claude = run_stream(
        EventParser::ClaudeStreamJson,
        &CLAUDE_STREAM[..2],
        Some(pricing_for("test-model")),
        AgentLimits::default(),
    )
    .await;
    assert_eq!(claude.usage.cost_source, Some(CostSource::Unavailable));
    assert_eq!(claude.usage.budget_tokens(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn partial_or_malformed_token_usage_is_not_reported() {
    let token_cap = AgentLimits {
        max_tokens: Some(1_000),
        require_enforceable_usage: true,
        ..AgentLimits::default()
    };

    let input_only = run_stream(
        EventParser::GenericJsonl,
        &[r#"{"type":"usage","usage":{"input_tokens":8}}"#],
        Some(pricing_for("test-model")),
        token_cap.clone(),
    )
    .await;
    assert_eq!(
        input_only.termination,
        AgentTerminationReason::BudgetUnverifiable
    );
    assert_eq!(input_only.usage.cost_source, Some(CostSource::Unavailable));
    assert_eq!(input_only.usage.budget_tokens(), 0);

    let malformed = run_stream(
        EventParser::CodexJsonl,
        &[r#"{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":40,"output_tokens":5}}"#],
        Some(pricing_for("test-model")),
        token_cap,
    )
    .await;
    assert_eq!(
        malformed.termination,
        AgentTerminationReason::BudgetUnverifiable
    );
    assert_eq!(malformed.usage.cost_source, Some(CostSource::Unavailable));
    assert_eq!(malformed.usage.budget_tokens(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn run_cost_cap_alone_requires_a_known_cost() {
    let run_capped = AgentLimits {
        run_cost_capped: true,
        require_enforceable_usage: true,
        ..AgentLimits::default()
    };

    let unpriced = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        None,
        run_capped.clone(),
    )
    .await;
    assert_eq!(
        unpriced.termination,
        AgentTerminationReason::BudgetUnverifiable
    );

    let priced = run_stream(
        EventParser::CodexJsonl,
        CODEX_STREAM,
        Some(pricing_for("test-model")),
        run_capped,
    )
    .await;
    assert_eq!(priced.termination, AgentTerminationReason::Completed);
    assert_eq!(priced.usage.cost_source, Some(CostSource::PricingTable));
}
