//! Provider configuration and factory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use forgetest_core::traits::LlmProvider;

use crate::anthropic::AnthropicProvider;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiProvider;

/// Configuration for a single LLM provider.
///
/// Note: Custom Debug impl masks API keys to prevent accidental exposure in logs.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ProviderConfig {
    OpenAI {
        api_key: String,
        #[serde(default)]
        base_url: Option<String>,
        #[serde(default)]
        org_id: Option<String>,
    },
    Anthropic {
        api_key: String,
        #[serde(default)]
        base_url: Option<String>,
    },
    Ollama {
        #[serde(default = "default_ollama_url")]
        base_url: String,
    },
}

/// Code runner implementation to use for compile/test execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunnerType {
    #[default]
    Local,
    Docker,
}

impl std::fmt::Display for RunnerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerType::Local => write!(f, "local"),
            RunnerType::Docker => write!(f, "docker"),
        }
    }
}

impl std::str::FromStr for RunnerType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(RunnerType::Local),
            "docker" => Ok(RunnerType::Docker),
            other => Err(format!("unknown runner: {other}")),
        }
    }
}

/// Configuration for code execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerConfig {
    /// Runner implementation (`local` or `docker`).
    #[serde(default, rename = "type")]
    pub runner_type: RunnerType,
    /// Docker image for the Docker runner.
    #[serde(default = "default_docker_image")]
    pub docker_image: String,
    /// Memory limit passed to Docker.
    #[serde(default = "default_docker_memory")]
    pub memory: String,
    /// CPU limit passed to Docker.
    #[serde(default = "default_docker_cpus")]
    pub cpus: f64,
    /// Process limit passed to Docker.
    #[serde(default = "default_docker_pids_limit")]
    pub pids_limit: u32,
    /// Network mode passed to Docker.
    #[serde(default = "default_docker_network")]
    pub network: String,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            runner_type: RunnerType::Local,
            docker_image: default_docker_image(),
            memory: default_docker_memory(),
            cpus: default_docker_cpus(),
            pids_limit: default_docker_pids_limit(),
            network: default_docker_network(),
        }
    }
}

fn default_docker_image() -> String {
    "forgetest-runner-rust:0.1.0".to_string()
}

fn default_docker_memory() -> String {
    "512m".to_string()
}

fn default_docker_cpus() -> f64 {
    1.0
}

fn default_docker_pids_limit() -> u32 {
    128
}

fn default_docker_network() -> String {
    "none".to_string()
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderConfig::OpenAI {
                api_key: _,
                base_url,
                org_id,
            } => f
                .debug_struct("OpenAI")
                .field("api_key", &"***")
                .field("base_url", base_url)
                .field("org_id", org_id)
                .finish(),
            ProviderConfig::Anthropic {
                api_key: _,
                base_url,
            } => f
                .debug_struct("Anthropic")
                .field("api_key", &"***")
                .field("base_url", base_url)
                .finish(),
            ProviderConfig::Ollama { base_url } => f
                .debug_struct("Ollama")
                .field("base_url", base_url)
                .finish(),
        }
    }
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

/// Top-level forgetest configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgetestConfig {
    /// Provider configurations keyed by name.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// Default provider to use.
    #[serde(default = "default_provider")]
    pub default_provider: String,
    /// Default model to use.
    #[serde(default = "default_model")]
    pub default_model: String,
    /// Default temperature (0.0 for deterministic evals).
    #[serde(default)]
    pub default_temperature: f64,
    /// Max retries on provider errors.
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    /// Delay between retries in milliseconds.
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
    /// Max concurrent eval runs.
    #[serde(default = "default_parallelism")]
    pub parallelism: usize,
    /// Output directory for results.
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,
    /// Code runner configuration.
    #[serde(default)]
    pub runner: RunnerConfig,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

/// Sentinel used when a provider model has not been selected explicitly.
pub const UNCONFIGURED_MODEL_ID: &str = "replace-with-provider-model-id";

fn default_model() -> String {
    UNCONFIGURED_MODEL_ID.to_string()
}
fn default_retries() -> u32 {
    3
}
fn default_retry_delay() -> u64 {
    1000
}
fn default_parallelism() -> usize {
    4
}
fn default_output_dir() -> PathBuf {
    PathBuf::from("./forgetest-results")
}

impl Default for ForgetestConfig {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: default_provider(),
            default_model: default_model(),
            default_temperature: 0.0,
            max_retries: default_retries(),
            retry_delay_ms: default_retry_delay(),
            parallelism: default_parallelism(),
            output_dir: default_output_dir(),
            runner: RunnerConfig::default(),
        }
    }
}

/// Resolve a documented credential environment reference.
fn resolve_api_key(value: &str) -> Result<String> {
    if !value.contains("${") {
        return Ok(value.to_string());
    }

    let variable = value
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .context("credential references must use the exact form ${VARIABLE}")?;
    anyhow::ensure!(
        matches!(
            variable,
            "FORGETEST_OPENAI_KEY"
                | "OPENAI_API_KEY"
                | "FORGETEST_ANTHROPIC_KEY"
                | "ANTHROPIC_API_KEY"
        ),
        "unsupported credential variable: {variable}"
    );
    std::env::var(variable).with_context(|| format!("credential variable {variable} is not set"))
}

/// Resolve env vars in a provider config.
fn resolve_provider_config(config: &ProviderConfig) -> Result<ProviderConfig> {
    match config {
        ProviderConfig::OpenAI {
            api_key,
            base_url,
            org_id,
        } => Ok(ProviderConfig::OpenAI {
            api_key: resolve_api_key(api_key)?,
            base_url: base_url.clone(),
            org_id: org_id.clone(),
        }),
        ProviderConfig::Anthropic { api_key, base_url } => Ok(ProviderConfig::Anthropic {
            api_key: resolve_api_key(api_key)?,
            base_url: base_url.clone(),
        }),
        ProviderConfig::Ollama { base_url } => Ok(ProviderConfig::Ollama {
            base_url: base_url.clone(),
        }),
    }
}

/// Load configuration from well-known paths.
///
/// Search order:
/// 1. Explicit path passed by the caller
/// 2. `~/.config/forgetest/config.toml`
///
/// Environment variable overrides: `FORGETEST_OPENAI_KEY`, `FORGETEST_ANTHROPIC_KEY`.
pub fn load_config() -> Result<ForgetestConfig> {
    load_config_from(None)
}

/// Load config from an explicit path, or search the default locations.
pub fn load_config_from(path: Option<&Path>) -> Result<ForgetestConfig> {
    let config_path = discover_config_path(path, user_home().as_deref())?;

    let mut config = match config_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config: {}", path.display()))?;
            toml::from_str::<ForgetestConfig>(&content)
                .with_context(|| format!("failed to parse config: {}", path.display()))?
        }
        None => ForgetestConfig::default(),
    };

    // Apply env var overrides
    if let Ok(key) = std::env::var("FORGETEST_ANTHROPIC_KEY") {
        config
            .providers
            .entry("anthropic".into())
            .or_insert(ProviderConfig::Anthropic {
                api_key: String::new(),
                base_url: None,
            });
        if let Some(ProviderConfig::Anthropic { api_key, .. }) =
            config.providers.get_mut("anthropic")
        {
            *api_key = key;
        }
    }

    if let Ok(key) = std::env::var("FORGETEST_OPENAI_KEY") {
        config
            .providers
            .entry("openai".into())
            .or_insert(ProviderConfig::OpenAI {
                api_key: String::new(),
                base_url: None,
                org_id: None,
            });
        if let Some(ProviderConfig::OpenAI { api_key, .. }) = config.providers.get_mut("openai") {
            *api_key = key;
        }
    }

    // Resolve only documented API-key references. URLs and other provider
    // fields remain literal so repository content cannot redirect credentials.
    let resolved: HashMap<String, ProviderConfig> = config
        .providers
        .iter()
        .map(|(name, provider)| {
            resolve_provider_config(provider)
                .map(|resolved| (name.clone(), resolved))
                .with_context(|| format!("invalid provider configuration: {name}"))
        })
        .collect::<Result<_>>()?;
    config.providers = resolved;

    Ok(config)
}

fn discover_config_path(path: Option<&Path>, home: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = path {
        if p.exists() {
            return Ok(Some(p.to_path_buf()));
        } else {
            anyhow::bail!("config file not found: {}", p.display());
        }
    }

    Ok(home
        .map(|home| home.join(".config").join("forgetest").join("config.toml"))
        .filter(|candidate| candidate.exists()))
}

fn user_home() -> Option<PathBuf> {
    // Use HOME on Unix, USERPROFILE on Windows
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Create a provider instance from its configuration.
pub fn create_provider(name: &str, config: &ProviderConfig) -> Result<Box<dyn LlmProvider>> {
    match config {
        ProviderConfig::Anthropic { api_key, base_url } => {
            Ok(Box::new(AnthropicProvider::new(api_key, base_url.clone())))
        }
        ProviderConfig::OpenAI {
            api_key,
            base_url,
            org_id,
        } => Ok(Box::new(OpenAiProvider::new(
            api_key,
            base_url.clone(),
            org_id.clone(),
        ))),
        ProviderConfig::Ollama { base_url } => {
            let _ = name;
            Ok(Box::new(OllamaProvider::new(base_url)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_env_vars_basic() {
        std::env::set_var("_FORGETEST_TEST_VAR", "hello");
        let error = resolve_api_key("${_FORGETEST_TEST_VAR}").unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported credential variable"));
        std::env::remove_var("_FORGETEST_TEST_VAR");
    }

    #[test]
    fn resolve_api_key_accepts_documented_variables() {
        std::env::set_var("FORGETEST_OPENAI_KEY", "test-key");
        assert_eq!(
            resolve_api_key("${FORGETEST_OPENAI_KEY}").unwrap(),
            "test-key"
        );
        std::env::remove_var("FORGETEST_OPENAI_KEY");
    }

    #[test]
    fn provider_urls_are_not_interpolated_from_environment() {
        std::env::set_var("_FORGETEST_TEST_HOST", "attacker.example");
        let provider = ProviderConfig::OpenAI {
            api_key: "literal".into(),
            base_url: Some("https://${_FORGETEST_TEST_HOST}/v1".into()),
            org_id: Some("${_FORGETEST_TEST_HOST}".into()),
        };

        let resolved = resolve_provider_config(&provider).unwrap();

        match resolved {
            ProviderConfig::OpenAI {
                base_url, org_id, ..
            } => {
                assert_eq!(
                    base_url.as_deref(),
                    Some("https://${_FORGETEST_TEST_HOST}/v1")
                );
                assert_eq!(org_id.as_deref(), Some("${_FORGETEST_TEST_HOST}"));
            }
            _ => panic!("wrong provider type"),
        }
        std::env::remove_var("_FORGETEST_TEST_HOST");
    }

    #[test]
    fn default_discovery_only_uses_user_config() {
        let home = tempfile::tempdir().unwrap();
        let config_dir = home.path().join(".config/forgetest");
        std::fs::create_dir_all(&config_dir).unwrap();
        let expected = config_dir.join("config.toml");
        std::fs::write(&expected, "").unwrap();

        assert_eq!(
            discover_config_path(None, Some(home.path())).unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn default_config() {
        let config = ForgetestConfig::default();
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.default_model, UNCONFIGURED_MODEL_ID);
        assert_eq!(config.parallelism, 4);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.runner.runner_type, RunnerType::Local);
        assert_eq!(config.runner.docker_image, "forgetest-runner-rust:0.1.0");
    }

    #[test]
    fn parse_provider_config() {
        let toml_str = r#"
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[providers.anthropic]
type = "anthropic"
api_key = "sk-test"

[providers.openai]
type = "openai"
api_key = "sk-openai"

[providers.ollama]
type = "ollama"
base_url = "http://localhost:11434"

[runner]
type = "docker"
docker_image = "forgetest-runner-rust:0.1.0"
"#;
        let config: ForgetestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.providers.len(), 3);
        assert_eq!(config.runner.runner_type, RunnerType::Docker);
        assert!(matches!(
            config.providers.get("anthropic"),
            Some(ProviderConfig::Anthropic { .. })
        ));
    }

    #[test]
    fn rejects_unknown_top_level_config_fields() {
        let error = toml::from_str::<ForgetestConfig>(
            r#"
default_provider = "anthropic"
trusted_host_command = "curl example.invalid"
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_runner_config_fields() {
        let error = toml::from_str::<ForgetestConfig>(
            r#"
[runner]
type = "docker"
privileged = true
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_provider_config_fields() {
        let error = toml::from_str::<ForgetestConfig>(
            r#"
[providers.openai]
type = "openai"
api_key = "test"
shell = "printenv"
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn parse_runner_type() {
        assert_eq!("local".parse::<RunnerType>().unwrap(), RunnerType::Local);
        assert_eq!("docker".parse::<RunnerType>().unwrap(), RunnerType::Docker);
        assert!("podman".parse::<RunnerType>().is_err());
    }
}
