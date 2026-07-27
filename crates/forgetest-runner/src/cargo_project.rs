//! Temporary Cargo project for compiling and testing generated code.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tempfile::TempDir;
use tokio::process::Command;

use forgetest_core::model::Language;
use forgetest_core::traits::Dependency;

/// A temporary Cargo project with no process-isolation guarantee.
///
/// On drop, the temporary directory is automatically cleaned up.
pub struct CargoProject {
    /// Temporary directory containing the Cargo project.
    work_dir: TempDir,
    /// Shared target directory for caching compiled dependencies.
    shared_target_dir: PathBuf,
    /// Timeout for compilation and test runs.
    timeout: Duration,
    /// Language being evaluated.
    language: Language,
}

impl CargoProject {
    /// Create a fresh temporary Cargo project.
    pub fn new(language: Language, timeout: Duration, shared_target_dir: &Path) -> Result<Self> {
        let work_dir = TempDir::new().context("failed to create temp directory")?;
        Self::from_temp_dir(work_dir, language, timeout, shared_target_dir)
    }

    /// Create a fresh temporary Cargo project under a specific parent directory.
    pub fn new_in(
        language: Language,
        timeout: Duration,
        shared_target_dir: &Path,
        temp_parent: &Path,
    ) -> Result<Self> {
        std::fs::create_dir_all(temp_parent)
            .with_context(|| format!("failed to create temp parent: {}", temp_parent.display()))?;
        let work_dir = tempfile::Builder::new()
            .prefix("forgetest-")
            .tempdir_in(temp_parent)
            .with_context(|| {
                format!(
                    "failed to create temp directory in {}",
                    temp_parent.display()
                )
            })?;
        Self::from_temp_dir(work_dir, language, timeout, shared_target_dir)
    }

    fn from_temp_dir(
        work_dir: TempDir,
        language: Language,
        timeout: Duration,
        shared_target_dir: &Path,
    ) -> Result<Self> {
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
        std::fs::create_dir_all(work_dir.path().join(".home"))
            .context("failed to create isolated home directory")?;
        std::fs::create_dir_all(work_dir.path().join(".cargo-home"))
            .context("failed to create isolated Cargo home")?;
        std::fs::create_dir_all(work_dir.path().join(".tmp"))
            .context("failed to create isolated temp directory")?;

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

    /// Get the path to the temporary working directory.
    pub fn work_dir(&self) -> &Path {
        self.work_dir.path()
    }

    /// Get the shared target directory path.
    pub fn shared_target_dir(&self) -> &Path {
        &self.shared_target_dir
    }

    /// Get the command timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the language being evaluated.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Write source code to the temporary project.
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

    /// Write test code into the temporary project.
    ///
    /// Appends the test code to `src/lib.rs` after the main source code.
    pub fn write_test(&self, test_code: &str) -> Result<()> {
        let lib_path = self.work_dir.path().join("src").join("lib.rs");
        let existing = std::fs::read_to_string(&lib_path).unwrap_or_default();
        let combined = format!("{existing}\n\n{test_code}");
        std::fs::write(&lib_path, combined).context("failed to write test code")?;
        Ok(())
    }

    /// Add a dependency to the temporary project's Cargo.toml.
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

    /// Configure a Cargo child with an explicit, credential-free environment.
    pub fn configure_command(&self, command: &mut Command) {
        let home = self.work_dir.path().join(".home");
        let cargo_home = self.work_dir.path().join(".cargo-home");
        let temp = self.work_dir.path().join(".tmp");

        command
            .env_clear()
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("CARGO_HOME", cargo_home)
            .env("CARGO_TARGET_DIR", &self.shared_target_dir)
            .env("CARGO_TERM_COLOR", "never")
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp);

        for variable in [
            "PATH",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "RUSTC",
            "RUSTDOC",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
            "NIX_SSL_CERT_FILE",
        ] {
            if let Some(value) = std::env::var_os(variable) {
                command.env(variable, value);
            }
        }

        if std::env::var_os("RUSTUP_HOME").is_none() {
            if let Some(original_home) = std::env::var_os("HOME") {
                let rustup_home = PathBuf::from(original_home).join(".rustup");
                if rustup_home.is_dir() {
                    command.env("RUSTUP_HOME", rustup_home);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_creates_valid_cargo_project() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        assert!(sandbox.work_dir().join("Cargo.toml").exists());
        assert!(sandbox.work_dir().join("src").join("lib.rs").exists());
    }

    #[test]
    fn write_source_lib() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox.write_source("pub fn hello() {}").unwrap();
        let content = std::fs::read_to_string(sandbox.work_dir().join("src/lib.rs")).unwrap();
        assert!(content.contains("pub fn hello"));
    }

    #[test]
    fn write_source_main() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

        sandbox
            .write_source("fn main() { println!(\"hi\"); }")
            .unwrap();
        assert!(sandbox.work_dir().join("src/main.rs").exists());
    }

    #[test]
    fn add_dependency() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

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
        let sandbox =
            CargoProject::new(Language::Rust, Duration::from_secs(60), target.path()).unwrap();

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
