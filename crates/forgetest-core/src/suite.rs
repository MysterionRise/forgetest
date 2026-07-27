//! Strict repository-suite loading and content identities.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Language;
use crate::repository_report::GraderCheckKind;

/// Current repository-suite schema version.
pub const SUITE_SCHEMA_VERSION: u32 = 2;
/// Current repository-task schema version.
pub const TASK_SCHEMA_VERSION: u32 = 1;

/// A fully resolved repository evaluation suite.
#[derive(Debug, Clone)]
pub struct ResolvedSuite {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub root: PathBuf,
    pub tasks: Vec<ResolvedRepositoryTask>,
    pub digest: String,
}

/// A repository task with validated absolute paths.
#[derive(Debug, Clone)]
pub struct ResolvedRepositoryTask {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: TaskCategory,
    pub language: Language,
    pub prompt: String,
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub grader: PathBuf,
    pub reference_patch: Option<PathBuf>,
    pub verifier: VerifierSpec,
    pub provenance: TaskProvenance,
    pub timeout_secs: u64,
    pub tags: Vec<String>,
    pub digest: String,
}

/// Portfolio task category used for corpus balancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskCategory {
    BugFix,
    Feature,
    ApiMigration,
    AsyncConcurrency,
    SecurityRobustness,
}

/// Command executed by the independent verifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierSpec {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default = "default_verifier_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub checks: Vec<VerifierCheckSpec>,
}

/// One named deterministic verifier command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierCheckSpec {
    pub name: String,
    pub kind: GraderCheckKind,
    pub command: Vec<String>,
}

/// Provenance attached to every published task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskProvenance {
    pub kind: ProvenanceKind,
    pub license: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_revision: Option<String>,
    #[serde(default)]
    pub audited_at: Option<String>,
}

/// Whether a task was authored for forgetest or adapted from a source snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    Authored,
    Snapshot,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteFile {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    tasks: Vec<SuiteTaskEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteTaskEntry {
    id: String,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskFile {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    category: TaskCategory,
    language: Language,
    prompt: String,
    workspace: String,
    grader: String,
    #[serde(default)]
    reference_patch: Option<String>,
    #[serde(default = "default_task_timeout")]
    timeout_secs: u64,
    #[serde(default)]
    tags: Vec<String>,
    verifier: VerifierSpec,
    provenance: TaskProvenance,
}

fn default_task_timeout() -> u64 {
    900
}

fn default_verifier_timeout() -> u64 {
    300
}

/// Load and strictly validate a repository suite.
pub fn load_suite(path: &Path) -> Result<ResolvedSuite> {
    let suite_content = fs::read_to_string(path)
        .with_context(|| format!("failed to read suite: {}", path.display()))?;
    let manifest: SuiteFile = toml::from_str(&suite_content)
        .with_context(|| format!("failed to parse suite: {}", path.display()))?;
    anyhow::ensure!(
        manifest.schema_version == SUITE_SCHEMA_VERSION,
        "unsupported suite schema version {}; expected {}",
        manifest.schema_version,
        SUITE_SCHEMA_VERSION
    );
    validate_identifier(&manifest.id, "suite ID")?;
    anyhow::ensure!(!manifest.name.trim().is_empty(), "suite name is empty");
    anyhow::ensure!(!manifest.tasks.is_empty(), "suite contains no tasks");

    let root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .with_context(|| format!("failed to resolve suite root: {}", path.display()))?;
    let mut seen = HashSet::new();
    let mut tasks = Vec::with_capacity(manifest.tasks.len());

    for entry in &manifest.tasks {
        validate_identifier(&entry.id, "task ID")?;
        anyhow::ensure!(
            seen.insert(entry.id.clone()),
            "duplicate task ID: {}",
            entry.id
        );
        let task_root = safe_existing_path(&root, &entry.path, "task path")?;
        anyhow::ensure!(
            task_root.is_dir(),
            "task path is not a directory: {}",
            entry.path
        );
        ensure_no_symlinks(&task_root)?;
        let task = load_task(&task_root, &entry.id)?;
        tasks.push(task);
    }

    let mut suite_hasher = Sha256::new();
    suite_hasher.update(b"forgetest-suite-v2\0");
    suite_hasher.update(manifest.id.as_bytes());
    suite_hasher.update(b"\0");
    suite_hasher.update(manifest.name.as_bytes());
    suite_hasher.update(b"\0");
    suite_hasher.update(manifest.description.as_bytes());
    for task in &tasks {
        suite_hasher.update(b"\0");
        suite_hasher.update(task.id.as_bytes());
        suite_hasher.update(b"\0");
        suite_hasher.update(task.digest.as_bytes());
    }

    Ok(ResolvedSuite {
        schema_version: manifest.schema_version,
        id: manifest.id,
        name: manifest.name,
        description: manifest.description,
        root,
        tasks,
        digest: hex_digest(suite_hasher.finalize()),
    })
}

fn load_task(root: &Path, expected_id: &str) -> Result<ResolvedRepositoryTask> {
    let task_path = root.join("task.toml");
    let task_content = fs::read_to_string(&task_path)
        .with_context(|| format!("failed to read task: {}", task_path.display()))?;
    let task: TaskFile = toml::from_str(&task_content)
        .with_context(|| format!("failed to parse task: {}", task_path.display()))?;

    anyhow::ensure!(
        task.schema_version == TASK_SCHEMA_VERSION,
        "task '{}' uses unsupported schema version {}; expected {}",
        expected_id,
        task.schema_version,
        TASK_SCHEMA_VERSION
    );
    anyhow::ensure!(
        task.id == expected_id,
        "task ID mismatch: suite declares '{}' but task.toml declares '{}'",
        expected_id,
        task.id
    );
    validate_identifier(&task.id, "task ID")?;
    anyhow::ensure!(
        !task.name.trim().is_empty(),
        "task '{}' name is empty",
        task.id
    );
    anyhow::ensure!(
        task.language == Language::Rust,
        "task '{}' uses {}; only Rust is supported in v1",
        task.id,
        task.language
    );
    anyhow::ensure!(
        task.timeout_secs > 0,
        "task '{}' timeout must be positive",
        task.id
    );
    let has_legacy_command = !task.verifier.command.is_empty();
    let has_named_checks = !task.verifier.checks.is_empty();
    anyhow::ensure!(
        has_legacy_command != has_named_checks,
        "task '{}' verifier must configure exactly one of command or checks",
        task.id
    );
    for check in &task.verifier.checks {
        anyhow::ensure!(
            !check.name.trim().is_empty(),
            "task '{}' verifier check name is empty",
            task.id
        );
        anyhow::ensure!(
            !check.command.is_empty(),
            "task '{}' verifier check '{}' command is empty",
            task.id,
            check.name
        );
    }
    anyhow::ensure!(
        task.verifier.timeout_secs > 0,
        "task '{}' verifier timeout must be positive",
        task.id
    );
    anyhow::ensure!(
        !task.provenance.license.trim().is_empty(),
        "task '{}' provenance license is empty",
        task.id
    );
    if task.provenance.kind == ProvenanceKind::Snapshot {
        let source_url = task
            .provenance
            .source_url
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        let source_revision = task
            .provenance
            .source_revision
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        anyhow::ensure!(
            source_url.is_some() && source_revision.is_some(),
            "snapshot task '{}' requires source_url and source_revision",
            task.id
        );
        let audited_at = task
            .provenance
            .audited_at
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .with_context(|| format!("snapshot task '{}' requires audited_at", task.id))?;
        chrono::NaiveDate::parse_from_str(audited_at, "%Y-%m-%d").with_context(|| {
            format!("snapshot task '{}' audited_at must use YYYY-MM-DD", task.id)
        })?;
    }

    let prompt_path = safe_existing_path(root, &task.prompt, "prompt")?;
    anyhow::ensure!(
        prompt_path.is_file(),
        "task '{}' prompt is not a file",
        task.id
    );
    let prompt = fs::read_to_string(&prompt_path)
        .with_context(|| format!("failed to read prompt: {}", prompt_path.display()))?;
    anyhow::ensure!(
        !prompt.trim().is_empty(),
        "task '{}' prompt is empty",
        task.id
    );

    let workspace = safe_existing_path(root, &task.workspace, "workspace")?;
    let grader = safe_existing_path(root, &task.grader, "grader")?;
    anyhow::ensure!(
        workspace.is_dir(),
        "task '{}' workspace is not a directory",
        task.id
    );
    anyhow::ensure!(
        grader.is_dir(),
        "task '{}' grader is not a directory",
        task.id
    );
    anyhow::ensure!(
        !grader.starts_with(&workspace) && !workspace.starts_with(&grader),
        "task '{}' workspace and grader must be separate",
        task.id
    );
    ensure_no_symlinks(&workspace)?;
    ensure_no_symlinks(&grader)?;

    let reference_patch = task
        .reference_patch
        .as_deref()
        .map(|relative| safe_existing_path(root, relative, "reference_patch"))
        .transpose()?;

    let mut hasher = Sha256::new();
    hasher.update(b"forgetest-task-v1\0");
    hasher.update(task_content.as_bytes());
    hash_file(&mut hasher, root, &prompt_path)?;
    hash_tree(&mut hasher, root, &workspace)?;
    hash_tree(&mut hasher, root, &grader)?;
    if let Some(patch) = &reference_patch {
        hash_file(&mut hasher, root, patch)?;
    }

    Ok(ResolvedRepositoryTask {
        schema_version: task.schema_version,
        id: task.id,
        name: task.name,
        description: task.description,
        category: task.category,
        language: task.language,
        prompt,
        root: root.to_path_buf(),
        workspace,
        grader,
        reference_patch,
        verifier: task.verifier,
        provenance: task.provenance,
        timeout_secs: task.timeout_secs,
        tags: task.tags,
        digest: hex_digest(hasher.finalize()),
    })
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "{label} is empty");
    anyhow::ensure!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "{label} '{value}' may contain only ASCII letters, numbers, '-' and '_'"
    );
    Ok(())
}

fn safe_existing_path(root: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    anyhow::ensure!(
        !relative_path.is_absolute()
            && relative_path
                .components()
                .all(|component| { matches!(component, Component::Normal(_) | Component::CurDir) }),
        "{label} must be a relative path without '..': {relative}"
    );
    let candidate = root.join(relative_path);
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("{label} does not exist: {}", candidate.display()))?;
    anyhow::ensure!(
        resolved.starts_with(root),
        "{label} escapes task root: {relative}"
    );
    Ok(resolved)
}

fn ensure_no_symlinks(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "symlink is not allowed: {}",
        root.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(root)? {
            ensure_no_symlinks(&entry?.path())?;
        }
    }
    Ok(())
}

fn hash_tree(hasher: &mut Sha256, task_root: &Path, path: &Path) -> Result<()> {
    let mut entries = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            hash_tree(hasher, task_root, &entry)?;
        } else {
            hash_file(hasher, task_root, &entry)?;
        }
    }
    Ok(())
}

fn hash_file(hasher: &mut Sha256, task_root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(task_root)
        .context("hashed path is outside task root")?;
    hasher.update(b"\0path\0");
    hasher.update(relative.to_string_lossy().replace('\\', "/").as_bytes());
    hasher.update(b"\0content\0");
    hasher.update(fs::read(path)?);
    Ok(())
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
