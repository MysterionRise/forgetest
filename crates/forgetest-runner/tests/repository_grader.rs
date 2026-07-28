use std::time::Duration;

use forgetest_core::agent::{GradeCheckRequest, GradeRequest, Grader};
use forgetest_core::repository_report::GraderCheckKind;
use forgetest_runner::{DockerRepositoryGrader, DockerVerifierConfig, LocalRepositoryGrader};
use uuid::Uuid;

#[cfg(not(windows))]
fn environment_command() -> Vec<String> {
    vec!["env".into()]
}

#[cfg(windows)]
fn environment_command() -> Vec<String> {
    vec!["cmd.exe".into(), "/C".into(), "set".into()]
}

#[cfg(not(windows))]
fn exit_command(code: u8) -> Vec<String> {
    vec!["sh".into(), "-c".into(), format!("exit {code}")]
}

#[cfg(windows)]
fn exit_command(code: u8) -> Vec<String> {
    vec!["cmd.exe".into(), "/C".into(), format!("exit /B {code}")]
}

#[tokio::test]
async fn local_grader_clears_inherited_secrets() {
    std::env::set_var("_FORGETEST_GRADER_SECRET", "must-not-leak");
    let workspace = tempfile::tempdir().unwrap();
    let grader = LocalRepositoryGrader::new(64 * 1024);
    let outcome = grader
        .grade(&GradeRequest {
            trial_id: Uuid::new_v4(),
            workspace: workspace.path().to_path_buf(),
            checks: vec![GradeCheckRequest {
                name: "environment".into(),
                kind: GraderCheckKind::Other,
                command: environment_command(),
            }],
            timeout: Duration::from_secs(5),
        })
        .await
        .unwrap();
    std::env::remove_var("_FORGETEST_GRADER_SECRET");

    assert!(outcome.success);
    assert!(!outcome.stdout.contains("_FORGETEST_GRADER_SECRET"));
}

#[cfg(unix)]
#[tokio::test]
async fn local_grader_kills_process_group_on_timeout() {
    let workspace = tempfile::tempdir().unwrap();
    let marker = workspace.path().join("escaped");
    let grader = LocalRepositoryGrader::new(64 * 1024);
    let result = grader
        .grade(&GradeRequest {
            trial_id: Uuid::new_v4(),
            workspace: workspace.path().to_path_buf(),
            checks: vec![GradeCheckRequest {
                name: "timeout".into(),
                kind: GraderCheckKind::Other,
                command: vec![
                    "sh".into(),
                    "-c".into(),
                    "(sleep 1; touch escaped) & wait".into(),
                ],
            }],
            timeout: Duration::from_millis(100),
        })
        .await;

    assert!(result.unwrap_err().to_string().contains("timed out"));
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(!marker.exists());
}

#[tokio::test]
async fn local_grader_records_named_check_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let grader = LocalRepositoryGrader::new(64 * 1024);

    let outcome = grader
        .grade(&GradeRequest {
            trial_id: Uuid::new_v4(),
            workspace: workspace.path().to_path_buf(),
            checks: vec![
                GradeCheckRequest {
                    name: "new behavior".into(),
                    kind: GraderCheckKind::FailToPass,
                    command: exit_command(0),
                },
                GradeCheckRequest {
                    name: "existing behavior".into(),
                    kind: GraderCheckKind::PassToPass,
                    command: exit_command(1),
                },
            ],
            timeout: Duration::from_secs(5),
        })
        .await
        .unwrap();

    assert!(!outcome.success);
    assert_eq!(outcome.checks.len(), 2);
    assert_eq!(outcome.checks[0].name, "new behavior");
    assert!(outcome.checks[0].passed);
    assert_eq!(outcome.checks[1].kind, GraderCheckKind::PassToPass);
    assert!(!outcome.checks[1].passed);
}

#[test]
fn docker_verifier_args_enforce_hardening() {
    let workspace = tempfile::tempdir().unwrap();
    let grader = DockerRepositoryGrader::new(DockerVerifierConfig::default());
    let args = grader.docker_args(
        "trial-name",
        workspace.path(),
        &["cargo".into(), "test".into(), "--locked".into()],
    );

    assert!(args.windows(2).any(|pair| pair == ["--network", "none"]));
    assert!(args.windows(2).any(|pair| pair == ["--cap-drop", "ALL"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--security-opt", "no-new-privileges"]));
    assert!(args.iter().any(|argument| argument == "--read-only"));
    assert!(args.iter().any(|argument| argument == "--rm"));
    assert!(args.windows(2).any(|pair| pair == ["--name", "trial-name"]));
    assert!(args
        .windows(2)
        .filter(|pair| pair[0] == "--mount")
        .all(|pair| !pair[1].ends_with(",rw")));
    assert!(args
        .windows(2)
        .any(|pair| pair[0] == "--tmpfs" && pair[1].starts_with("/tmp:")));
    assert!(args
        .windows(2)
        .any(|pair| pair[0] == "--tmpfs" && pair[1].starts_with("/work/target:")));
    assert!(args
        .windows(2)
        .filter(|pair| pair[0] == "--tmpfs" && pair[1].starts_with("/work/target:"))
        .all(|pair| {
            let options: Vec<_> = pair[1].split(',').collect();
            options.contains(&"exec") && !options.contains(&"noexec")
        }));
    assert!(!args.iter().any(|argument| argument.contains("docker.sock")));
}
