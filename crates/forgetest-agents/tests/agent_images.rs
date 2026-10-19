//! Gated checks for the agent image recipes in `docker/agents`.
//!
//! Build the images with `scripts/build-agent-images.sh`, then run:
//!
//! ```text
//! FORGETEST_AGENT_IMAGE_TEST=1 \
//! FORGETEST_CODEX_IMAGE=REGISTRY/forgetest-codex@sha256:DIGEST \
//! FORGETEST_CLAUDE_IMAGE=REGISTRY/forgetest-claude@sha256:DIGEST \
//! cargo test -p forgetest-agents --test agent_images
//! ```

use forgetest_agents::{
    doctor_container, is_immutable_image, CommandProfile, DockerAgentConfig, DockerProcessAgent,
    EventParser, MemoryEventSink,
};
use forgetest_core::agent::{
    AgentEventKind, AgentExecutor, AgentIdentity, AgentLimits, AgentOutcome, AgentRequest,
    AgentTerminationReason,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// Native binary pins from docker/agents/codex.Dockerfile and claude.Dockerfile,
// with the exact `--version` line each binary prints.
const CODEX_VERSION: &str = "codex-cli 0.163.0";
const CODEX_SHA256: &str = "399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215";
const CLAUDE_VERSION: &str = "2.1.302 (Claude Code)";
const CLAUDE_SHA256: &str = "60d7eb7f12fbb28a52533fe8be5884097403562aa02dc73991a021c7067e29a5";

// Where codex.Dockerfile installs the vendor package, and the system config it
// writes.
const CODEX_PACKAGE: &str = "/opt/codex-0.163.0";
const CODEX_CONFIG: &str = "check_for_update_on_startup = false\n\n[analytics]\nenabled = false\n";

fn image_under_test(variable: &str) -> Option<String> {
    match std::env::var("FORGETEST_AGENT_IMAGE_TEST").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("0") => return None,
        Ok("1") => {}
        other => panic!("FORGETEST_AGENT_IMAGE_TEST must be 1, 0 or unset, not {other:?}"),
    }
    let image = std::env::var(variable)
        .unwrap_or_else(|_| panic!("{variable} must name the agent image under test"));
    assert!(
        is_immutable_image(&image),
        "{variable} must use NAME@sha256:DIGEST: {image}"
    );
    Some(image)
}

async fn assert_preflight(profile: CommandProfile, image: &str, version: &str, sha256: &str) {
    let report = doctor_container(&profile, image).await.unwrap();

    assert_eq!(
        report.executable_path,
        format!("/usr/local/bin/{}", profile.executable)
    );
    assert_eq!(report.executable_sha256, sha256);
    // The preflight appends container stderr after the CLI's stdout, so the
    // first line is the version the binary itself printed.
    assert_eq!(
        report.version.lines().next(),
        Some(version),
        "unexpected {} version report: {:?}",
        profile.executable,
        report.version
    );
}

/// Run `sh -c SCRIPT ARGS...` with forgetest's agent container flags: a
/// read-only root, all capabilities dropped, the workspace owner's non-root
/// UID, and `HOME=/tmp/home` on an empty tmpfs. The network is disabled.
async fn run_in_agent_container(
    label: &str,
    image: &str,
    script: &str,
    arguments: &[&str],
) -> AgentOutcome {
    let workspace = tempfile::tempdir().unwrap();
    let mut shell_arguments = vec!["-c".to_string(), script.to_string()];
    shell_arguments.extend(arguments.iter().map(|argument| argument.to_string()));
    let profile = CommandProfile {
        name: label.into(),
        executable: "sh".into(),
        arguments: shell_arguments,
        version_arguments: vec!["--version".into()],
        parser: EventParser::Text,
        environment_allowlist: Vec::new(),
    };
    let agent = DockerProcessAgent::new(
        profile,
        AgentIdentity {
            adapter: label.into(),
            adapter_version: "test".into(),
            executable_sha256: None,
            model: "none".into(),
            configuration_digest: "agent-image-runtime-contract".into(),
        },
        DockerAgentConfig {
            image: image.into(),
            network: "none".into(),
            ..DockerAgentConfig::default()
        },
    );

    agent
        .execute(
            &AgentRequest {
                trial_id: Uuid::new_v4(),
                task_id: "agent-image-runtime-contract".into(),
                prompt: String::new(),
                workspace: workspace.path().to_path_buf(),
                limits: AgentLimits {
                    timeout_secs: 120,
                    ..AgentLimits::default()
                },
            },
            &MemoryEventSink::default(),
        )
        .await
        .unwrap()
}

/// Stdout lines of a container run; container stderr arrives as one warning
/// event and is left out.
fn stdout_lines(outcome: &AgentOutcome) -> Vec<&str> {
    outcome
        .events
        .iter()
        .filter(|event| event.kind == AgentEventKind::Message)
        .map(|event| event.message.as_str())
        .collect()
}

/// Run `EXECUTABLE --version` under the runtime contract with HOME not yet
/// created.
async fn assert_runtime_contract(executable: &str, image: &str, version: &str) {
    let outcome = run_in_agent_container(
        &format!("{executable}-runtime-contract"),
        image,
        concat!(
            "[ \"$(id -u)\" != 0 ] && [ \"$HOME\" = /tmp/home ] && [ ! -e \"$HOME\" ] ",
            "|| exit 70; exec \"$0\" --version"
        ),
        &[executable],
    )
    .await;

    assert_eq!(
        outcome.termination,
        AgentTerminationReason::Completed,
        "{executable} --version failed under the runtime contract: {outcome:#?}"
    );
    assert!(
        outcome.events.iter().any(|event| event.message == version),
        "{executable} --version did not report {version}: {outcome:#?}"
    );
}

#[tokio::test]
async fn codex_image_preflight_reports_pinned_native_binary() {
    let Some(image) = image_under_test("FORGETEST_CODEX_IMAGE") else {
        return;
    };
    assert_preflight(
        CommandProfile::codex("preflight"),
        &image,
        CODEX_VERSION,
        CODEX_SHA256,
    )
    .await;
}

#[tokio::test]
async fn claude_image_preflight_reports_pinned_native_binary() {
    let Some(image) = image_under_test("FORGETEST_CLAUDE_IMAGE") else {
        return;
    };
    assert_preflight(
        CommandProfile::claude("preflight"),
        &image,
        CLAUDE_VERSION,
        CLAUDE_SHA256,
    )
    .await;
}

#[tokio::test]
async fn codex_image_meets_runtime_contract() {
    let Some(image) = image_under_test("FORGETEST_CODEX_IMAGE") else {
        return;
    };
    assert_runtime_contract("codex", &image, CODEX_VERSION).await;
}

#[tokio::test]
async fn claude_image_meets_runtime_contract() {
    let Some(image) = image_under_test("FORGETEST_CLAUDE_IMAGE") else {
        return;
    };
    assert_runtime_contract("claude", &image, CLAUDE_VERSION).await;
}

#[tokio::test]
async fn codex_image_installs_vendor_package_with_helpers() {
    let Some(image) = image_under_test("FORGETEST_CODEX_IMAGE") else {
        return;
    };
    let outcome = run_in_agent_container(
        "codex-package",
        &image,
        concat!(
            "package=\"$0\"; ",
            "[ \"$(id -u)\" != 0 ] || { echo 'running as root' >&2; exit 70; }; ",
            "target=$(readlink -f /usr/local/bin/codex); ",
            "[ \"$target\" = \"$package/bin/codex\" ] ",
            "|| { echo \"codex resolves to $target\" >&2; exit 71; }; ",
            "for helper in bin/codex-code-mode-host codex-path/rg codex-resources/bwrap; do ",
            "[ -f \"$package/$helper\" ] && [ -x \"$package/$helper\" ] ",
            "|| { echo \"$helper is missing or not executable\" >&2; exit 72; }; ",
            "done; ",
            "exec \"$package/codex-path/rg\" --version"
        ),
        &[CODEX_PACKAGE],
    )
    .await;

    assert_eq!(
        outcome.termination,
        AgentTerminationReason::Completed,
        "Codex package check failed: {outcome:#?}"
    );
    assert!(
        stdout_lines(&outcome)
            .first()
            .is_some_and(|line| line.starts_with("ripgrep ")),
        "packaged rg did not report a version: {outcome:#?}"
    );
}

#[tokio::test]
async fn claude_image_disables_updates_and_telemetry() {
    let Some(image) = image_under_test("FORGETEST_CLAUDE_IMAGE") else {
        return;
    };
    let outcome = run_in_agent_container(
        "claude-settings",
        &image,
        concat!(
            "printf '%s\\n' ",
            "\"DISABLE_AUTOUPDATER=${DISABLE_AUTOUPDATER-unset}\" ",
            "\"DISABLE_TELEMETRY=${DISABLE_TELEMETRY-unset}\" ",
            "\"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=",
            "${CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC-unset}\""
        ),
        &[],
    )
    .await;

    assert_eq!(outcome.termination, AgentTerminationReason::Completed);
    assert_eq!(
        stdout_lines(&outcome),
        [
            "DISABLE_AUTOUPDATER=1",
            "DISABLE_TELEMETRY=1",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1",
        ]
    );
}

#[tokio::test]
async fn codex_image_disables_updates_and_analytics() {
    let Some(image) = image_under_test("FORGETEST_CODEX_IMAGE") else {
        return;
    };
    let outcome = run_in_agent_container(
        "codex-settings",
        &image,
        "sha256sum /etc/codex/config.toml && cat /etc/codex/config.toml",
        &[],
    )
    .await;

    assert_eq!(
        outcome.termination,
        AgentTerminationReason::Completed,
        "could not read /etc/codex/config.toml: {outcome:#?}"
    );
    // The event stream drops blank lines, so exact bytes are compared by hash.
    let expected = format!("{:x}", Sha256::digest(CODEX_CONFIG.as_bytes()));
    let lines = stdout_lines(&outcome);
    assert_eq!(
        lines
            .first()
            .and_then(|line| line.split_whitespace().next()),
        Some(expected.as_str()),
        "/etc/codex/config.toml differs from the two expected settings: {lines:#?}"
    );
}

#[test]
fn agent_images_set_no_entrypoint_or_fixed_user() {
    let (Some(codex), Some(claude)) = (
        image_under_test("FORGETEST_CODEX_IMAGE"),
        image_under_test("FORGETEST_CLAUDE_IMAGE"),
    ) else {
        return;
    };
    for image in [codex, claude] {
        let output = std::process::Command::new("docker")
            .args([
                "image",
                "inspect",
                "--format",
                "{{json .Config.Entrypoint}} {{json .Config.User}}",
                &image,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "docker image inspect failed for {image}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let observed = String::from_utf8(output.stdout).unwrap();
        assert!(
            matches!(observed.trim(), "null \"\"" | "[] \"\""),
            "{image} sets an entrypoint or a fixed user: {observed}"
        );
    }
}
