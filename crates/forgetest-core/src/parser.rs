//! TOML eval case parser.
//!
//! Loads eval sets from TOML files and directories, and validates them.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::{EvalCase, EvalSet, Expectations, Language};
use crate::traits::Dependency;

/// Intermediate TOML structure for parsing eval set files.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlEvalFile {
    eval_set: TomlEvalSetHeader,
    #[serde(default)]
    cases: Vec<TomlEvalCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlEvalSetHeader {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_language_str")]
    default_language: String,
    #[serde(default = "default_timeout")]
    default_timeout_secs: u64,
}

fn default_language_str() -> String {
    "rust".to_string()
}

fn default_timeout() -> u64 {
    60
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlEvalCase {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    prompt: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    dependencies: Vec<TomlDependency>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    expectations: Option<TomlExpectations>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlDependency {
    name: String,
    version: String,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlExpectations {
    #[serde(default = "default_true")]
    should_compile: bool,
    #[serde(default = "default_true")]
    should_pass_tests: bool,
    #[serde(default)]
    test_file: Option<String>,
    #[serde(default)]
    expected_functions: Vec<String>,
    #[serde(default)]
    expected_types: Vec<String>,
    #[serde(default)]
    max_clippy_warnings: Option<u32>,
    #[serde(default)]
    custom_check: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Parse a single TOML file into an `EvalSet`.
pub fn parse_eval_set(path: &Path) -> Result<EvalSet> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read eval set file: {}", path.display()))?;

    parse_eval_set_str(&content, path)
}

/// Parse a TOML string into an `EvalSet` (useful for testing).
pub fn parse_eval_set_str(content: &str, source_path: &Path) -> Result<EvalSet> {
    let parsed: TomlEvalFile = toml::from_str(content)
        .with_context(|| format!("failed to parse TOML: {}", source_path.display()))?;

    let default_language: Language = parsed
        .eval_set
        .default_language
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{}", e))?;

    let cases = parsed
        .cases
        .into_iter()
        .map(|c| {
            let language = c
                .language
                .map(|l| l.parse().map_err(|e: String| anyhow::anyhow!("{}", e)))
                .transpose()?;

            let expectations = match c.expectations {
                Some(exp) => Expectations {
                    should_compile: exp.should_compile,
                    should_pass_tests: exp.should_pass_tests,
                    test_file: exp.test_file,
                    expected_functions: exp.expected_functions,
                    expected_types: exp.expected_types,
                    max_clippy_warnings: exp.max_clippy_warnings,
                    custom_check: exp.custom_check,
                },
                None => Expectations::default(),
            };

            let dependencies = c
                .dependencies
                .into_iter()
                .map(|d| Dependency {
                    name: d.name,
                    version: d.version,
                    features: d.features,
                })
                .collect();

            Ok(EvalCase {
                id: c.id,
                name: c.name,
                description: c.description,
                prompt: c.prompt,
                language,
                context: vec![],
                expectations,
                tags: c.tags,
                dependencies,
                timeout_secs: c.timeout_secs,
                max_tokens: c.max_tokens,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let set = EvalSet {
        id: parsed.eval_set.id,
        name: parsed.eval_set.name,
        description: parsed.eval_set.description,
        cases,
        default_language,
        default_timeout_secs: parsed.eval_set.default_timeout_secs,
    };
    validate_parsed_eval_set(&set)?;
    Ok(set)
}

fn validate_parsed_eval_set(set: &EvalSet) -> Result<()> {
    anyhow::ensure!(!set.id.trim().is_empty(), "eval set ID is empty");
    anyhow::ensure!(!set.name.trim().is_empty(), "eval set name is empty");
    anyhow::ensure!(!set.cases.is_empty(), "eval set '{}' has no cases", set.id);
    anyhow::ensure!(
        set.default_timeout_secs > 0,
        "eval set '{}' default timeout must be positive",
        set.id
    );

    let mut case_ids = std::collections::HashSet::new();
    for case in &set.cases {
        anyhow::ensure!(!case.id.trim().is_empty(), "case ID is empty");
        anyhow::ensure!(case_ids.insert(&case.id), "duplicate case ID: {}", case.id);
        anyhow::ensure!(
            !case.name.trim().is_empty(),
            "case '{}' name is empty",
            case.id
        );
        anyhow::ensure!(
            !case.prompt.trim().is_empty(),
            "case '{}' prompt is empty",
            case.id
        );
        anyhow::ensure!(
            case.timeout_secs.is_none_or(|timeout| timeout > 0),
            "case '{}' timeout must be positive",
            case.id
        );
        anyhow::ensure!(
            case.max_tokens.is_none_or(|max_tokens| max_tokens > 0),
            "case '{}' max_tokens must be positive",
            case.id
        );
        let mut dependency_names = std::collections::HashSet::new();
        for dependency in &case.dependencies {
            anyhow::ensure!(
                !dependency.name.trim().is_empty() && !dependency.version.trim().is_empty(),
                "case '{}' dependencies require non-empty names and versions",
                case.id
            );
            anyhow::ensure!(
                dependency_names.insert(&dependency.name),
                "case '{}' has duplicate dependency '{}'",
                case.id,
                dependency.name
            );
        }
    }
    Ok(())
}

/// Recursively load all `.toml` eval set files from a directory.
pub fn load_eval_directory(dir: &Path) -> Result<Vec<EvalSet>> {
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }

    let mut paths = Vec::new();
    collect_eval_files(dir, &mut paths)?;
    paths.sort();
    let mut sets = Vec::with_capacity(paths.len());
    let mut ids = std::collections::HashSet::new();
    for path in paths {
        let set = parse_eval_set(&path)?;
        anyhow::ensure!(
            ids.insert(set.id.clone()),
            "duplicate eval set ID '{}' while loading {}",
            set.id,
            dir.display()
        );
        sets.push(set);
    }

    Ok(sets)
}

fn collect_eval_files(dir: &Path, paths: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?
    {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "eval directory cannot contain symlinks: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_eval_files(&path, paths)?;
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
            paths.push(path);
        }
    }
    Ok(())
}

/// A warning from eval set validation.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The case ID (if applicable).
    pub case_id: Option<String>,
    /// Warning message.
    pub message: String,
}

/// An error from eval set validation.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The case ID (if applicable).
    pub case_id: Option<String>,
    /// Error message.
    pub message: String,
}

/// Validate an eval set for common issues.
pub fn validate_eval_set(set: &EvalSet) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    // Check for duplicate case IDs
    let mut seen_ids = std::collections::HashSet::new();
    for case in &set.cases {
        if !seen_ids.insert(&case.id) {
            warnings.push(ValidationWarning {
                case_id: Some(case.id.clone()),
                message: format!("duplicate case ID: {}", case.id),
            });
        }
    }

    // Check for should_pass_tests=true without test_file
    for case in &set.cases {
        if case.expectations.should_pass_tests && case.expectations.test_file.is_none() {
            warnings.push(ValidationWarning {
                case_id: Some(case.id.clone()),
                message: "should_pass_tests is true but no test_file provided".into(),
            });
        }
    }

    // Check for empty prompts
    for case in &set.cases {
        if case.prompt.trim().is_empty() {
            warnings.push(ValidationWarning {
                case_id: Some(case.id.clone()),
                message: "prompt is empty".into(),
            });
        }
    }

    warnings
}

/// Validate an eval set for issues that prevent execution in this version.
pub fn validate_eval_set_errors(set: &EvalSet) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    if set.default_language != Language::Rust {
        errors.push(ValidationError {
            case_id: None,
            message: format!(
                "only Rust execution is supported in v0.1; default language is {}",
                set.default_language
            ),
        });
    }

    for case in &set.cases {
        if !seen_ids.insert(&case.id) {
            errors.push(ValidationError {
                case_id: Some(case.id.clone()),
                message: format!("duplicate case ID: {}", case.id),
            });
        }
        if case.prompt.trim().is_empty() {
            errors.push(ValidationError {
                case_id: Some(case.id.clone()),
                message: "prompt is empty".into(),
            });
        }
        if case
            .language
            .is_some_and(|language| language != Language::Rust)
        {
            errors.push(ValidationError {
                case_id: Some(case.id.clone()),
                message: "only Rust execution is supported in v0.1".into(),
            });
        }
        if case.expectations.custom_check.is_some() {
            errors.push(ValidationError {
                case_id: Some(case.id.clone()),
                message: "custom_check is unsupported in v0.1".into(),
            });
        }
        if !case.expectations.should_compile {
            errors.push(ValidationError {
                case_id: Some(case.id.clone()),
                message: "should_compile = false is unsupported; snippet tasks must compile".into(),
            });
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const VALID_TOML: &str = r#"
[eval_set]
id = "test-set"
name = "Test Set"
description = "A test eval set"
default_language = "rust"
default_timeout_secs = 60

[[cases]]
id = "fibonacci"
name = "Fibonacci function"
description = "Write a fibonacci function"
prompt = """
Write a Rust function `fn fibonacci(n: u64) -> u64` that returns
the nth Fibonacci number.
"""
tags = ["algorithms", "basics"]

[cases.expectations]
should_compile = true
should_pass_tests = true
test_file = """
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fib() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(10), 55);
    }
}
"""
expected_functions = ["fibonacci"]
"#;

    #[test]
    fn parse_valid_toml() {
        let set = parse_eval_set_str(VALID_TOML, &PathBuf::from("test.toml")).unwrap();
        assert_eq!(set.id, "test-set");
        assert_eq!(set.name, "Test Set");
        assert_eq!(set.cases.len(), 1);
        assert_eq!(set.cases[0].id, "fibonacci");
        assert!(set.cases[0].expectations.test_file.is_some());
        assert_eq!(
            set.cases[0].expectations.expected_functions,
            vec!["fibonacci"]
        );
    }

    #[test]
    fn parse_missing_optional_fields() {
        let toml = r#"
[eval_set]
id = "minimal"
name = "Minimal"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write hello world"
"#;
        let set = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap();
        assert_eq!(set.default_language, Language::Rust);
        assert_eq!(set.default_timeout_secs, 60);
        assert!(set.cases[0].expectations.should_compile);
        assert!(set.cases[0].tags.is_empty());
    }

    #[test]
    fn parse_rejects_duplicate_ids() {
        let toml = r#"
[eval_set]
id = "dupes"
name = "Dupes"

[[cases]]
id = "same"
name = "First"
prompt = "Write something"

[[cases]]
id = "same"
name = "Second"
prompt = "Write something else"
"#;
        let error = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap_err();
        assert!(error.to_string().contains("duplicate case ID"));
    }

    #[test]
    fn validate_tests_without_test_file() {
        let toml = r#"
[eval_set]
id = "no-tests"
name = "No Tests"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write something"

[cases.expectations]
should_pass_tests = true
"#;
        let set = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap();
        let warnings = validate_eval_set(&set);
        assert!(warnings.iter().any(|w| w.message.contains("no test_file")));
    }

    #[test]
    fn validate_custom_check_is_error() {
        let toml = r#"
[eval_set]
id = "custom"
name = "Custom"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write something"

[cases.expectations]
should_pass_tests = false
custom_check = "grep something"
"#;
        let set = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap();
        let warnings = validate_eval_set(&set);
        let errors = validate_eval_set_errors(&set);

        assert!(warnings.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("unsupported in v0.1"));
    }

    #[test]
    fn validate_expected_compile_failure_is_unsupported() {
        let toml = r#"
[eval_set]
id = "negative-compile"
name = "Negative compile"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write invalid code"

[cases.expectations]
should_compile = false
should_pass_tests = false
"#;
        let set = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap();
        let errors = validate_eval_set_errors(&set);

        assert!(errors
            .iter()
            .any(|error| error.message.contains("should_compile = false")));
    }

    #[test]
    fn parse_malformed_toml() {
        let bad = "this is not [valid toml }{";
        let result = parse_eval_set_str(bad, &PathBuf::from("bad.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn load_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.toml");
        std::fs::write(&file_path, VALID_TOML).unwrap();

        let sets = load_eval_directory(dir.path()).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].id, "test-set");
    }

    #[test]
    fn load_directory_fails_on_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("valid.toml"), VALID_TOML).unwrap();
        std::fs::write(dir.path().join("broken.toml"), "not valid = [toml").unwrap();

        let error = load_eval_directory(dir.path()).unwrap_err();

        assert!(error.to_string().contains("broken.toml"));
    }

    #[test]
    fn load_directory_rejects_duplicate_eval_set_ids() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("first.toml"), VALID_TOML).unwrap();
        std::fs::write(dir.path().join("second.toml"), VALID_TOML).unwrap();

        let error = load_eval_directory(dir.path()).unwrap_err();

        assert!(error.to_string().contains("duplicate eval set ID"));
    }

    #[test]
    fn validate_rejects_non_rust_cases() {
        let toml = r#"
[eval_set]
id = "python"
name = "Python"
default_language = "python"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write Python"
"#;
        let set = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap();
        let errors = validate_eval_set_errors(&set);

        assert!(errors.iter().any(|e| e.message.contains("only Rust")));
    }

    #[test]
    fn parse_rejects_empty_prompts() {
        let toml = r#"
[eval_set]
id = "empty"
name = "Empty"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "   "
"#;
        let error = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap_err();
        assert!(error.to_string().contains("prompt is empty"));
    }

    #[test]
    fn parse_rejects_unknown_task_fields() {
        let toml = r#"
[eval_set]
id = "strict"
name = "Strict"

[[cases]]
id = "case1"
name = "Case 1"
prompt = "Write something"
promt = "misspelled"
"#;
        let error = parse_eval_set_str(toml, &PathBuf::from("test.toml")).unwrap_err();
        assert!(format!("{error:#}").contains("unknown field `promt`"));
    }
}
