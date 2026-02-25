//! Core data model types for forgetest.
//!
//! These are the fundamental types that the entire forgetest system uses
//! to represent eval cases, expectations, and eval sets.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::traits::Dependency;

/// A single evaluation task sent to an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    /// Unique identifier for this eval case.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of what this eval case tests.
    #[serde(default)]
    pub description: String,
    /// The prompt sent to the LLM.
    pub prompt: String,
    /// The programming language expected in the response.
    #[serde(default)]
    pub language: Option<Language>,
    /// Additional files provided as context to the LLM.
    #[serde(default)]
    pub context: Vec<ContextFile>,
    /// What we check about the LLM's output.
    #[serde(default)]
    pub expectations: Expectations,
    /// Tags for filtering eval cases.
    #[serde(default)]
    pub tags: Vec<String>,
    /// External crate dependencies needed to compile this case.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Per-case timeout override in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Per-case max tokens override.
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// A file provided as context to the LLM alongside the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    /// Relative path (e.g. "src/lib.rs").
    pub path: String,
    /// File contents.
    pub content: String,
}

/// What we check about the LLM's generated code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expectations {
    /// Whether the generated code should compile successfully.
    #[serde(default = "default_true")]
    pub should_compile: bool,
    /// Whether the generated code should pass tests.
    #[serde(default = "default_true")]
    pub should_pass_tests: bool,
    /// Test code to compile and run against the generated output.
    #[serde(default)]
    pub test_file: Option<String>,
    /// Function names that must exist in the generated code.
    #[serde(default)]
    pub expected_functions: Vec<String>,
    /// Type names that must exist in the generated code.
    #[serde(default)]
    pub expected_types: Vec<String>,
    /// Maximum allowed clippy warnings (None = no limit).
    #[serde(default)]
    pub max_clippy_warnings: Option<u32>,
    /// Shell command that receives generated code on stdin; exits 0 for pass.
    #[serde(default)]
    pub custom_check: Option<String>,
}

impl Default for Expectations {
    fn default() -> Self {
        Self {
            should_compile: true,
            should_pass_tests: true,
            test_file: None,
            expected_functions: Vec::new(),
            expected_types: Vec::new(),
            max_clippy_warnings: None,
            custom_check: None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    Go,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::Python => write!(f, "python"),
            Language::TypeScript => write!(f, "typescript"),
            Language::Go => write!(f, "go"),
        }
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" => Ok(Language::Rust),
            "python" => Ok(Language::Python),
            "typescript" | "ts" => Ok(Language::TypeScript),
            "go" | "golang" => Ok(Language::Go),
            other => Err(format!("unknown language: {other}")),
        }
    }
}

/// A collection of eval cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSet {
    /// Unique identifier for this eval set.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of this eval set.
    #[serde(default)]
    pub description: String,
    /// The eval cases in this set.
    #[serde(default)]
    pub cases: Vec<EvalCase>,
    /// Default language for cases that don't specify one.
    #[serde(default = "default_language")]
    pub default_language: Language,
    /// Default timeout in seconds for cases that don't specify one.
    #[serde(default = "default_timeout")]
    pub default_timeout_secs: u64,
}

fn default_language() -> Language {
    Language::Rust
}

fn default_timeout() -> u64 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_display_and_parse() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Python.to_string(), "python");
        assert_eq!("rust".parse::<Language>().unwrap(), Language::Rust);
        assert_eq!(
            "TypeScript".parse::<Language>().unwrap(),
            Language::TypeScript
        );
        assert_eq!("ts".parse::<Language>().unwrap(), Language::TypeScript);
        assert_eq!("golang".parse::<Language>().unwrap(), Language::Go);
        assert!("java".parse::<Language>().is_err());
    }

    #[test]
    fn expectations_default() {
        let exp = Expectations::default();
        assert!(exp.should_compile);
        assert!(exp.should_pass_tests);
        assert!(exp.test_file.is_none());
        assert!(exp.expected_functions.is_empty());
    }

    #[test]
    fn eval_case_serde_roundtrip() {
        let case = EvalCase {
            id: "test-1".into(),
            name: "Test Case".into(),
            description: "A test".into(),
            prompt: "Write hello world".into(),
            language: Some(Language::Rust),
            context: vec![],
            expectations: Expectations::default(),
            tags: vec!["basics".into()],
            dependencies: vec![],
            timeout_secs: Some(30),
            max_tokens: None,
        };
        let json = serde_json::to_string(&case).unwrap();
        let deserialized: EvalCase = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-1");
        assert_eq!(deserialized.language, Some(Language::Rust));
    }
}
