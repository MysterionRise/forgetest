//! Narrow Harbor task bridge for Rust tasks exported by forgetest.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::model::Language;
use crate::repository_report::GraderCheckKind;
use crate::suite::{
    ProvenanceKind, ResolvedSuite, TaskCategory, TaskProvenance, VerifierCheckSpec, VerifierSpec,
};

/// Metadata required when importing an audited Harbor bridge task.
#[derive(Debug, Clone)]
pub struct HarborImportMetadata {
    pub suite_id: String,
    pub suite_name: String,
    pub source_url: String,
    pub source_revision: String,
    pub license: String,
}

/// Export all tasks in a suite to Harbor's directory layout.
///
/// The bridge intentionally targets the forgetest-marked Rust subset rather
/// than claiming general Harbor environment compatibility.
pub fn export_suite_to_harbor(
    suite: &ResolvedSuite,
    output: &Path,
    base_image: &str,
) -> Result<()> {
    anyhow::ensure!(
        is_immutable_image(base_image),
        "Harbor export base image must use an immutable image with a complete SHA-256"
    );
    fs::create_dir_all(output)?;

    for task in &suite.tasks {
        let task_output = output.join(&task.id);
        let environment = task_output.join("environment");
        let tests = task_output.join("tests");
        fs::create_dir_all(&environment)?;
        fs::create_dir_all(&tests)?;
        fs::write(task_output.join("instruction.md"), &task.prompt)?;

        let mut metadata = BTreeMap::new();
        metadata.insert(
            "category".into(),
            toml::Value::String("software_engineering".into()),
        );
        metadata.insert(
            "tags".into(),
            toml::Value::Array(
                std::iter::once(toml::Value::String("rust".into()))
                    .chain(task.tags.iter().cloned().map(toml::Value::String))
                    .collect(),
            ),
        );
        metadata.insert("expert_time_estimate_hours".into(), toml::Value::Float(0.5));
        metadata.insert(
            "difficulty_explanation".into(),
            toml::Value::String("Repository change graded by deterministic Rust tests.".into()),
        );
        metadata.insert(
            "solution_explanation".into(),
            toml::Value::String("See the optional reference patch exported with this task.".into()),
        );
        metadata.insert(
            "verification_explanation".into(),
            toml::Value::String(
                "Hidden tests are overlaid after the agent exits and the configured Cargo command must succeed."
                    .into(),
            ),
        );
        metadata.insert(
            "forgetest_bridge_version".into(),
            toml::Value::String("1".into()),
        );
        metadata.insert(
            "forgetest_task_id".into(),
            toml::Value::String(task.id.clone()),
        );
        metadata.insert(
            "forgetest_category".into(),
            toml::Value::String(category_name(task.category).into()),
        );
        metadata.insert(
            "forgetest_verifier_command".into(),
            toml::Value::Array(
                task.verifier
                    .command
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            ),
        );
        metadata.insert(
            "forgetest_verifier_checks".into(),
            verifier_checks_value(&task.verifier.checks),
        );
        let harbor = HarborTaskFile {
            schema_version: "1.0".into(),
            metadata,
            verifier: HarborVerifier {
                timeout_sec: task.verifier.timeout_secs as f64,
            },
            agent: HarborAgent {
                timeout_sec: task.timeout_secs as f64,
            },
            environment: HarborEnvironment {
                build_timeout_sec: 600.0,
                docker_image: base_image.into(),
                cpus: 2.0,
                memory_mb: 2048,
                storage_mb: 10_240,
                allow_internet: false,
            },
        };
        fs::write(
            task_output.join("task.toml"),
            toml::to_string_pretty(&harbor)?,
        )?;
        fs::write(
            environment.join("Dockerfile"),
            format!("FROM {base_image}\nWORKDIR /app\nCOPY workspace/ /app/\n"),
        )?;
        copy_tree(&task.workspace, &environment.join("workspace"))?;
        copy_tree(&task.grader, &tests.join("hidden"))?;
        fs::write(tests.join("test.sh"), harbor_test_script(&task.verifier))?;

        if let Some(reference_patch) = &task.reference_patch {
            let solution = task_output.join("solution");
            fs::create_dir_all(&solution)?;
            fs::copy(reference_patch, solution.join("reference.patch"))?;
            fs::write(
                solution.join("solve.sh"),
                "#!/bin/sh\nset -eu\ncd /app\ngit apply /solution/reference.patch\n",
            )?;
            set_executable(&solution.join("solve.sh"))?;
        }
        set_executable(&tests.join("test.sh"))?;
    }
    Ok(())
}

/// Import only tasks carrying the marker emitted by `export_suite_to_harbor`.
pub fn import_harbor_task(
    source: &Path,
    output: &Path,
    metadata: &HarborImportMetadata,
) -> Result<()> {
    validate_identifier(&metadata.suite_id, "suite ID")?;
    anyhow::ensure!(
        !metadata.suite_name.trim().is_empty(),
        "suite name is empty"
    );
    anyhow::ensure!(
        !metadata.source_url.trim().is_empty()
            && !metadata.source_revision.trim().is_empty()
            && !metadata.license.trim().is_empty(),
        "Harbor import provenance fields cannot be empty"
    );
    let task_content = fs::read_to_string(source.join("task.toml"))
        .with_context(|| format!("missing Harbor task.toml in {}", source.display()))?;
    let harbor: toml::Value = toml::from_str(&task_content)?;
    anyhow::ensure!(
        harbor.get("schema_version").and_then(toml::Value::as_str) == Some("1.0"),
        "unsupported Harbor schema version"
    );
    let bridge_version = harbor
        .get("metadata")
        .and_then(|value| value.get("forgetest_bridge_version"))
        .and_then(toml::Value::as_str);
    anyhow::ensure!(
        bridge_version == Some("1"),
        "Harbor import requires the supported forgetest bridge marker"
    );
    let task_id = required_metadata_string(&harbor, "forgetest_task_id")?;
    validate_identifier(task_id, "task ID")?;
    validate_harbor_environment(&harbor)?;
    let category = parse_category(required_metadata_string(&harbor, "forgetest_category")?)?;
    let command = optional_metadata_string_array(&harbor, "forgetest_verifier_command")?;
    let checks = parse_verifier_checks(&harbor)?;
    anyhow::ensure!(
        !command.is_empty() || !checks.is_empty(),
        "Harbor bridge task has no verifier command or named checks"
    );
    anyhow::ensure!(
        command.is_empty() || checks.is_empty(),
        "Harbor bridge task cannot configure both a verifier command and named checks"
    );
    let verifier_timeout = harbor
        .get("verifier")
        .and_then(|value| value.get("timeout_sec"))
        .and_then(toml_number)
        .unwrap_or(300.0) as u64;
    let agent_timeout = harbor
        .get("agent")
        .and_then(|value| value.get("timeout_sec"))
        .and_then(toml_number)
        .unwrap_or(900.0) as u64;
    anyhow::ensure!(
        verifier_timeout > 0 && agent_timeout > 0,
        "Harbor timeouts must be positive"
    );

    let workspace_source = source.join("environment/workspace");
    let grader_source = source.join("tests/hidden");
    anyhow::ensure!(
        workspace_source.is_dir() && grader_source.is_dir(),
        "supported Harbor bridge task requires environment/workspace and tests/hidden"
    );
    let prompt = fs::read_to_string(source.join("instruction.md"))
        .context("supported Harbor bridge task is missing instruction.md")?;
    anyhow::ensure!(
        !prompt.trim().is_empty(),
        "Harbor task instruction is empty"
    );
    let task_root = output.join("tasks").join(task_id);
    fs::create_dir_all(&task_root)?;
    fs::write(task_root.join("prompt.md"), prompt)?;
    copy_tree(&workspace_source, &task_root.join("workspace"))?;
    copy_tree(&grader_source, &task_root.join("grader"))?;

    let reference_source = source.join("solution/reference.patch");
    let reference_patch = if reference_source.is_file() {
        fs::copy(&reference_source, task_root.join("reference.patch"))?;
        Some("reference.patch".into())
    } else {
        None
    };
    let imported_task = BridgeTaskFile {
        schema_version: 1,
        id: task_id.into(),
        name: task_id.replace(['-', '_'], " "),
        description: "Imported from the supported forgetest Harbor bridge subset.".into(),
        category,
        language: Language::Rust,
        prompt: "prompt.md".into(),
        workspace: "workspace".into(),
        grader: "grader".into(),
        reference_patch,
        timeout_secs: agent_timeout,
        tags: vec!["harbor-import".into()],
        verifier: VerifierSpec {
            command,
            timeout_secs: verifier_timeout,
            checks,
        },
        provenance: TaskProvenance {
            kind: ProvenanceKind::Snapshot,
            license: metadata.license.clone(),
            source_url: Some(metadata.source_url.clone()),
            source_revision: Some(metadata.source_revision.clone()),
            audited_at: Some(chrono::Utc::now().date_naive().to_string()),
        },
    };
    fs::write(
        task_root.join("task.toml"),
        toml::to_string_pretty(&imported_task)?,
    )?;
    let suite = BridgeSuiteFile {
        schema_version: 2,
        id: metadata.suite_id.clone(),
        name: metadata.suite_name.clone(),
        description: "Imported through the constrained forgetest Harbor bridge.".into(),
        tasks: vec![BridgeSuiteEntry {
            id: task_id.into(),
            path: format!("tasks/{task_id}"),
        }],
    };
    fs::write(output.join("suite.toml"), toml::to_string_pretty(&suite)?)?;
    Ok(())
}

#[derive(Serialize)]
struct HarborTaskFile {
    schema_version: String,
    metadata: BTreeMap<String, toml::Value>,
    verifier: HarborVerifier,
    agent: HarborAgent,
    environment: HarborEnvironment,
}

#[derive(Serialize)]
struct HarborVerifier {
    timeout_sec: f64,
}

#[derive(Serialize)]
struct HarborAgent {
    timeout_sec: f64,
}

#[derive(Serialize)]
struct HarborEnvironment {
    build_timeout_sec: f64,
    docker_image: String,
    cpus: f64,
    memory_mb: u64,
    storage_mb: u64,
    allow_internet: bool,
}

#[derive(Serialize)]
struct BridgeTaskFile {
    schema_version: u32,
    id: String,
    name: String,
    description: String,
    category: TaskCategory,
    language: Language,
    prompt: String,
    workspace: String,
    grader: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_patch: Option<String>,
    timeout_secs: u64,
    tags: Vec<String>,
    verifier: VerifierSpec,
    provenance: TaskProvenance,
}

#[derive(Serialize)]
struct BridgeSuiteFile {
    schema_version: u32,
    id: String,
    name: String,
    description: String,
    tasks: Vec<BridgeSuiteEntry>,
}

#[derive(Serialize)]
struct BridgeSuiteEntry {
    id: String,
    path: String,
}

fn harbor_test_script(verifier: &VerifierSpec) -> String {
    let commands = if verifier.command.is_empty() {
        verifier
            .checks
            .iter()
            .map(|check| check.command.as_slice())
            .collect::<Vec<_>>()
    } else {
        vec![verifier.command.as_slice()]
    };
    let checks = commands
        .into_iter()
        .map(|command| {
            let command = command
                .iter()
                .map(|part| shell_quote(part))
                .collect::<Vec<_>>()
                .join(" ");
            format!("if [ \"$status\" -eq 0 ]; then {command} || status=$?; fi\n")
        })
        .collect::<String>();
    format!(
        "#!/bin/sh\nset -u\nmkdir -p /logs/verifier\ncp -R /tests/hidden/. /app/\ncd /app\nstatus=0\n{checks}if [ \"$status\" -eq 0 ]; then printf '1\\n' > /logs/verifier/reward.txt; else printf '0\\n' > /logs/verifier/reward.txt; fi\nexit \"$status\"\n"
    )
}

fn verifier_checks_value(checks: &[VerifierCheckSpec]) -> toml::Value {
    toml::Value::Array(
        checks
            .iter()
            .map(|check| {
                let mut table = toml::map::Map::new();
                table.insert("name".into(), toml::Value::String(check.name.clone()));
                table.insert(
                    "kind".into(),
                    toml::Value::String(check_kind_name(check.kind).into()),
                );
                table.insert(
                    "command".into(),
                    toml::Value::Array(
                        check
                            .command
                            .iter()
                            .cloned()
                            .map(toml::Value::String)
                            .collect(),
                    ),
                );
                toml::Value::Table(table)
            })
            .collect(),
    )
}

fn optional_metadata_string_array(value: &toml::Value, key: &str) -> Result<Vec<String>> {
    let Some(values) = value.get("metadata").and_then(|metadata| metadata.get(key)) else {
        return Ok(Vec::new());
    };
    values
        .as_array()
        .with_context(|| format!("Harbor bridge metadata {key} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("Harbor bridge metadata {key} must contain strings"))
        })
        .collect()
}

fn parse_verifier_checks(value: &toml::Value) -> Result<Vec<VerifierCheckSpec>> {
    let Some(values) = value
        .get("metadata")
        .and_then(|metadata| metadata.get("forgetest_verifier_checks"))
    else {
        return Ok(Vec::new());
    };
    values
        .as_array()
        .context("Harbor bridge verifier checks must be an array")?
        .iter()
        .map(|value| {
            let table = value
                .as_table()
                .context("Harbor bridge verifier check must be a table")?;
            let name = table
                .get("name")
                .and_then(toml::Value::as_str)
                .context("Harbor bridge verifier check is missing name")?
                .to_string();
            let kind = table
                .get("kind")
                .and_then(toml::Value::as_str)
                .context("Harbor bridge verifier check is missing kind")
                .and_then(parse_check_kind)?;
            let command = table
                .get("command")
                .and_then(toml::Value::as_array)
                .context("Harbor bridge verifier check is missing command")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .context("Harbor bridge verifier check command must contain strings")
                })
                .collect::<Result<Vec<_>>>()?;
            anyhow::ensure!(
                !name.trim().is_empty() && !command.is_empty(),
                "Harbor bridge verifier checks require non-empty names and commands"
            );
            Ok(VerifierCheckSpec {
                name,
                kind,
                command,
            })
        })
        .collect()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn validate_harbor_environment(value: &toml::Value) -> Result<()> {
    let environment = value
        .get("environment")
        .and_then(toml::Value::as_table)
        .context("supported Harbor bridge task is missing [environment]")?;
    anyhow::ensure!(
        environment
            .get("allow_internet")
            .and_then(toml::Value::as_bool)
            == Some(false),
        "Harbor bridge supports only network-disabled task environments"
    );
    let image = environment
        .get("docker_image")
        .and_then(toml::Value::as_str)
        .context("Harbor bridge environment is missing docker_image")?;
    anyhow::ensure!(
        is_immutable_image(image),
        "Harbor bridge environment image must include a complete SHA-256"
    );
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "{label} '{value}' may contain only ASCII letters, numbers, '-' and '_'"
    );
    Ok(())
}

fn is_immutable_image(value: &str) -> bool {
    let Some((name, digest)) = value.rsplit_once("@sha256:") else {
        return false;
    };
    !name.is_empty()
        && !name.contains(char::is_whitespace)
        && digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn required_metadata_string<'a>(value: &'a toml::Value, key: &str) -> Result<&'a str> {
    value
        .get("metadata")
        .and_then(|value| value.get(key))
        .and_then(toml::Value::as_str)
        .with_context(|| format!("Harbor bridge metadata is missing {key}"))
}

fn toml_number(value: &toml::Value) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|value| value as f64))
}

fn category_name(category: TaskCategory) -> &'static str {
    match category {
        TaskCategory::BugFix => "bug_fix",
        TaskCategory::Feature => "feature",
        TaskCategory::ApiMigration => "api_migration",
        TaskCategory::AsyncConcurrency => "async_concurrency",
        TaskCategory::SecurityRobustness => "security_robustness",
    }
}

fn parse_category(value: &str) -> Result<TaskCategory> {
    match value {
        "bug_fix" => Ok(TaskCategory::BugFix),
        "feature" => Ok(TaskCategory::Feature),
        "api_migration" => Ok(TaskCategory::ApiMigration),
        "async_concurrency" => Ok(TaskCategory::AsyncConcurrency),
        "security_robustness" => Ok(TaskCategory::SecurityRobustness),
        _ => anyhow::bail!("unsupported forgetest task category: {value}"),
    }
}

fn check_kind_name(kind: GraderCheckKind) -> &'static str {
    match kind {
        GraderCheckKind::FailToPass => "fail_to_pass",
        GraderCheckKind::PassToPass => "pass_to_pass",
        GraderCheckKind::Compile => "compile",
        GraderCheckKind::Clippy => "clippy",
        GraderCheckKind::Other => "other",
    }
}

fn parse_check_kind(value: &str) -> Result<GraderCheckKind> {
    match value {
        "fail_to_pass" => Ok(GraderCheckKind::FailToPass),
        "pass_to_pass" => Ok(GraderCheckKind::PassToPass),
        "compile" => Ok(GraderCheckKind::Compile),
        "clippy" => Ok(GraderCheckKind::Clippy),
        "other" => Ok(GraderCheckKind::Other),
        _ => anyhow::bail!("unsupported forgetest verifier check kind: {value}"),
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "bridge copy source must be a real directory: {}",
        source.display()
    );
    fs::create_dir_all(destination)?;
    let mut entries = fs::read_dir(source)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "Harbor bridge does not copy symlinks: {}",
            path.display()
        );
        let target = destination.join(path.file_name().context("path has no file name")?);
        if metadata.is_dir() {
            copy_tree(&path, &target)?;
        } else if metadata.is_file() {
            fs::copy(&path, &target)?;
            fs::set_permissions(&target, metadata.permissions())?;
        } else {
            anyhow::bail!("unsupported bridge file type: {}", path.display());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_: &Path) -> Result<()> {
    Ok(())
}
