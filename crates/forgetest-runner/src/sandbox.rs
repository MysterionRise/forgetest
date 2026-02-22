//! Sandboxed Cargo project for compiling and testing generated code.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tempfile::TempDir;

use forgetest_core::model::Language;
use forgetest_core::traits::Dependency;

/// A sandboxed Cargo project for compiling and testing generated code.
///
/// On drop, the temporary directory is automatically cleaned up.
pub struct Sandbox {
    /// Temporary directory containing the Cargo project.
    work_dir: TempDir,
    /// Shared target directory for caching compiled dependencies.
    shared_target_dir: PathBuf,
    /// Timeout for compilation and test runs.
    timeout: Duration,
    /// Language being evaluated.
    language: Language,
}

impl Sandbox {
    /// Create a new sandbox with a fresh Cargo project.
    pub fn new(language: Language, timeout: Duration, shared_target_dir: &Path) -> Result<Self> {
        let work_dir = TempDir::new().context("failed to create temp directory")?;

        // Create a basic Cargo project
        let cargo_toml = r#"[package]
name = "eval_target"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
        std::fs::write(work_dir.path().join("Cargo.toml"), cargo_toml)
            .context("failed to write Cargo.toml")?;

        std::fs::create_dir_all(work_dir.path().join("src"))
            .context("failed to create src directory")?;

        std::fs::write(work_dir.path().join("src").join("lib.rs"), "")
            .context("failed to write lib.rs")?;

        // Ensure shared target dir exists
        std::fs::create_dir_all(shared_target_dir)
            .context("failed to create shared target directory")?;

        Ok(Self {
            work_dir,
            shared_target_dir: shared_target_dir.to_path_buf(),
            timeout,
            language,
        })
    }

    /// Get the path to the sandbox working directory.
    pub fn work_dir(&self) -> &Path {
        self.work_dir.path()
    }

    /// Get the shared target directory path.
    pub fn shared_target_dir(&self) -> &Path {
        &self.shared_target_dir
    }

    /// Get the sandbox timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the language being evaluated.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Write source code to the sandbox.
    ///
    /// If the code contains `fn main`, it goes to `src/main.rs`.
    /// Otherwise it goes to `src/lib.rs`.
    pub fn write_source(&self, code: &str) -> Result<()> {
        let filename = if code.contains("fn main") {
            "main.rs"
        } else {
            "lib.rs"
        };
        std::fs::write(self.work_dir.path().join("src").join(filename), code)
            .with_context(|| format!("failed to write src/{filename}"))?;
        Ok(())
    }

    /// Write test code into the sandbox.
    ///
    /// Appends the test code to `src/lib.rs` after the main source code.
    pub fn write_test(&self, test_code: &str) -> Result<()> {
        let lib_path = self.work_dir.path().join("src").join("lib.rs");
        let existing = std::fs::read_to_string(&lib_path).unwrap_or_default();
        let combined = format!("{existing}\n\n{test_code}");
        std::fs::write(&lib_path, combined).context("failed to write test code")?;
        Ok(())
    }

    /// Add a dependency to the sandbox's Cargo.toml.
    pub fn add_dependency(&self, dep: &Dependency) -> Result<()> {
        let cargo_path = self.work_dir.path().join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_path)?;
        let mut doc = content
            .parse::<toml_edit::DocumentMut>()
            .context("failed to parse Cargo.toml")?;

        let deps = doc["dependencies"]
            .as_table_mut()
            .context("missing [dependencies] table")?;

        if dep.features.is_empty() {
            deps[&dep.name] = toml_edit::value(&dep.version);
        } else {
            let mut table = toml_edit::InlineTable::new();
            table.insert("version", dep.version.clone().into());
            let mut features = toml_edit::Array::new();
            for f in &dep.features {
                features.push(f.as_str());
            }
            table.insert("features", toml_edit::Value::Array(features));
            deps[&dep.name] = toml_edit::value(table);
        }

        std::fs::write(&cargo_path, doc.to_string()).context("failed to update Cargo.toml")?;
        Ok(())
    }

    /// Build environment variables for child processes.
    ///
    /// Sets CARGO_TARGET_DIR and restricts access to sensitive env vars.
    pub fn build_env(&self) -> Vec<(String, String)> {
        let mut env = vec![(
            "CARGO_TARGET_DIR".to_string(),
            self.shared_target_dir.to_string_lossy().to_string(),
        )];

        // Clear sensitive env vars to prevent leakage into sandboxed code
        for var in &[
            "SSH_AUTH_SOCK",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "CARGO_REGISTRY_TOKEN",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "FORGETEST_ANTHROPIC_KEY",
            "FORGETEST_OPENAI_KEY",
            "DOCKER_HOST",
            "DOCKER_CONFIG",
            "KUBECONFIG",
            "DATABASE_URL",
            "NPM_TOKEN",
        ] {
            env.push((var.to_string(), String::new()));
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_creates_valid_cargo_project() {
        let target = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        assert!(sandbox.work_dir().join("Cargo.toml").exists());
        assert!(sandbox.work_dir().join("src").join("lib.rs").exists());
    }

    #[test]
    fn write_source_lib() {
        let target = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox.write_source("pub fn hello() {}").unwrap();
        let content = std::fs::read_to_string(sandbox.work_dir().join("src/lib.rs")).unwrap();
        assert!(content.contains("pub fn hello"));
    }

    #[test]
    fn write_source_main() {
        let target = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox
            .write_source("fn main() { println!(\"hi\"); }")
            .unwrap();
        assert!(sandbox.work_dir().join("src/main.rs").exists());
    }

    #[test]
    fn add_dependency() {
        let target = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox
            .add_dependency(&Dependency {
                name: "serde".into(),
                version: "1".into(),
                features: vec!["derive".into()],
            })
            .unwrap();

        let content = std::fs::read_to_string(sandbox.work_dir().join("Cargo.toml")).unwrap();
        assert!(content.contains("serde"));
    }

    #[test]
    fn write_test_appends() {
        let target = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox
            .write_source("pub fn add(a: i32, b: i32) -> i32 { a + b }")
            .unwrap();
        sandbox
            .write_test("#[test] fn test_add() { assert_eq!(add(1, 2), 3); }")
            .unwrap();

        let content = std::fs::read_to_string(sandbox.work_dir().join("src/lib.rs")).unwrap();
        assert!(content.contains("pub fn add"));
        assert!(content.contains("test_add"));
    }
}
