use forgetest_agents::{
    profile_configuration_digest, BenchmarkLock, CommandProfile, ContainerDoctorReport,
};

const LOCK: &str = r#"
schema_version = 1
created_at = "2026-07-27T12:00:00Z"
suite_digest = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
policy_digest = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
verifier_image = "runner@sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"

[[agents]]
name = "codex"
model = "gpt-exact"
cli_version = "codex-cli 1.2.3"
executable_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
configuration_digest = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
effort = "high"
container_image = "codex-agent@sha256:1111111111111111111111111111111111111111111111111111111111111111"

[[agents]]
name = "claude"
model = "claude-exact"
cli_version = "Claude Code 4.5.6"
executable_sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
configuration_digest = "abababababababababababababababababababababababababababababababab"
container_image = "claude-agent@sha256:2222222222222222222222222222222222222222222222222222222222222222"
"#;

#[test]
fn lock_requires_exact_suite_policy_and_agents() {
    let lock = BenchmarkLock::parse(LOCK).unwrap();

    lock.validate(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        &["codex", "claude"],
    )
    .unwrap();
    assert_eq!(lock.agent("codex").unwrap().model, "gpt-exact");
    assert_eq!(lock.agent("codex").unwrap().effort.as_deref(), Some("high"));
    assert_eq!(
        lock.agent("codex").unwrap().container_image,
        "codex-agent@sha256:1111111111111111111111111111111111111111111111111111111111111111"
    );

    assert!(lock
        .validate(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            &["codex"],
        )
        .unwrap_err()
        .to_string()
        .contains("suite digest"));
    assert!(lock
        .validate(
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &["codex"],
        )
        .unwrap_err()
        .to_string()
        .contains("policy digest"));
    assert!(lock
        .validate(
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            &["missing"],
        )
        .unwrap_err()
        .to_string()
        .contains("not present"));
}

#[test]
fn lock_rejects_mutable_agent_images() {
    let mutable = LOCK.replace(
        "codex-agent@sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "codex-agent:latest",
    );
    let error = BenchmarkLock::parse(&mutable).unwrap_err();

    assert!(error.to_string().contains("immutable container image"));
}

#[test]
fn lock_rejects_truncated_content_digests() {
    let malformed = LOCK.replace(
        "suite_digest = \"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\"",
        "suite_digest = \"abc\"",
    );
    let error = BenchmarkLock::parse(&malformed).unwrap_err();

    assert!(error.to_string().contains("suite digest"));
}

#[test]
fn lock_rejects_duplicate_agent_names() {
    let duplicated = LOCK.replace("name = \"claude\"", "name = \"codex\"");
    let error = BenchmarkLock::parse(&duplicated).unwrap_err();

    assert!(error.to_string().contains("duplicate locked agent"));
}

#[test]
fn lock_rejects_model_aliases_instead_of_freezing_them() {
    for alias in [
        "default",
        "latest",
        "sonnet",
        "opus",
        "haiku",
        "claude-sonnet-latest",
        "provider/default",
        "gpt-auto",
        "recommended:model",
    ] {
        let aliased = LOCK.replace("model = \"gpt-exact\"", &format!("model = \"{alias}\""));
        let error = BenchmarkLock::parse(&aliased).unwrap_err();

        assert!(
            error.to_string().contains("exact model ID"),
            "unexpected error for alias {alias}: {error:#}"
        );
    }
}

#[test]
fn locked_agent_rejects_in_image_version_or_binary_drift() {
    let lock = BenchmarkLock::parse(LOCK).unwrap();
    let codex = lock.agent("codex").unwrap();
    let observed = ContainerDoctorReport {
        executable_path: "/usr/local/bin/codex".into(),
        executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .into(),
        version: "codex-cli 1.2.3".into(),
    };

    codex.verify_container(&observed).unwrap();

    let changed = ContainerDoctorReport {
        version: "codex-cli 1.2.4".into(),
        ..observed
    };
    assert!(codex
        .verify_container(&changed)
        .unwrap_err()
        .to_string()
        .contains("version"));
}

#[test]
fn locked_agent_rejects_adapter_profile_drift() {
    let mut lock = BenchmarkLock::parse(LOCK).unwrap();
    let image = lock.agent("codex").unwrap().container_image.clone();
    let profile = CommandProfile::codex("gpt-exact");
    lock.agents[0].configuration_digest = profile_configuration_digest(&profile, &image);

    lock.agent("codex")
        .unwrap()
        .verify_profile(&profile)
        .unwrap();

    let mut changed = profile;
    changed.arguments.push("--new-behavior".into());
    let error = lock
        .agent("codex")
        .unwrap()
        .verify_profile(&changed)
        .unwrap_err();
    assert!(error.to_string().contains("profile differs"));
}
