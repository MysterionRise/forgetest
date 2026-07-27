use forgetest_agents::{
    builtin_profile, docker_preflight_arguments, doctor, parse_container_doctor_output,
    profile_configuration_digest, CommandProfile, DirectWorkspaceEnvironment, DockerAgentConfig,
    DockerProcessAgent, EventParser, MemoryEventSink, ProcessAgent, ScriptedAgent, ScriptedEdit,
};
use forgetest_core::agent::{
    AgentExecutor, AgentIdentity, AgentLimits, AgentRequest, AgentTerminationReason,
    WorkspaceEnvironment,
};
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

    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
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
    let profile = CommandProfile::generic(script.to_string_lossy(), EventParser::Text);
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
        "#!/bin/sh\nprintf 'working\\n'\nsleep 2\nprintf 'done\\n'\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    let agent = Arc::new(ProcessAgent::new(
        CommandProfile::generic(script.to_string_lossy(), EventParser::Text),
        identity("generic"),
    ));
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

    tokio::time::timeout(std::time::Duration::from_secs(1), sink.notify.notified())
        .await
        .expect("first event was buffered until process exit");
    assert!(
        sink.seen.load(Ordering::SeqCst),
        "first event was buffered until process exit"
    );
    assert_eq!(
        running.await.unwrap().unwrap().termination,
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
