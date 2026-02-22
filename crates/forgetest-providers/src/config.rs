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
#[serde(tag = "type", rename_all = "lowercase")]
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
}

fn default_provider() -> String {
    "anthropic".to_string()
}
fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
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
        }
    }
}

/// Resolve environment variable references like `${VAR_NAME}` in a string.
fn resolve_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}

/// Resolve env vars in a provider config.
fn resolve_provider_config(config: &ProviderConfig) -> ProviderConfig {
    match config {
        ProviderConfig::OpenAI {
            api_key,
            base_url,
            org_id,
        } => ProviderConfig::OpenAI {
            api_key: resolve_env_vars(api_key),
            base_url: base_url.as_ref().map(|u| resolve_env_vars(u)),
            org_id: org_id.as_ref().map(|o| resolve_env_vars(o)),
        },
        ProviderConfig::Anthropic { api_key, base_url } => ProviderConfig::Anthropic {
            api_key: resolve_env_vars(api_key),
            base_url: base_url.as_ref().map(|u| resolve_env_vars(u)),
        },
        ProviderConfig::Ollama { base_url } => ProviderConfig::Ollama {
            base_url: resolve_env_vars(base_url),
        },
    }
}

/// Load configuration from well-known paths.
///
/// Search order:
/// 1. `forgetest.toml` in the current directory
/// 2. `~/.config/forgetest/config.toml`
///
/// Environment variable overrides: `FORGETEST_OPENAI_KEY`, `FORGETEST_ANTHROPIC_KEY`.
pub fn load_config() -> Result<ForgetestConfig> {
    load_config_from(None)
}

/// Load config from an explicit path, or search the default locations.
pub fn load_config_from(path: Option<&Path>) -> Result<ForgetestConfig> {
    let config_path = if let Some(p) = path {
        if p.exists() {
            Some(p.to_path_buf())
        } else {
            anyhow::bail!("config file not found: {}", p.display());
        }
    } else {
        let local = PathBuf::from("forgetest.toml");
        if local.exists() {
            Some(local)
        } else if let Some(home) = dirs_path() {
            let global = home.join("config.toml");
            if global.exists() {
                Some(global)
            } else {
                None
            }
        } else {
            None
        }
    };

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

    // Resolve env vars in all provider configs
    let resolved: HashMap<String, ProviderConfig> = config
        .providers
        .iter()
        .map(|(k, v)| (k.clone(), resolve_provider_config(v)))
        .collect();
    config.providers = resolved;

    Ok(config)
}

fn dirs_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config").join("forgetest"))
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
        assert_eq!(resolve_env_vars("${_FORGETEST_TEST_VAR}"), "hello");
        assert_eq!(
            resolve_env_vars("prefix_${_FORGETEST_TEST_VAR}_suffix"),
            "prefix_hello_suffix"
        );
        std::env::remove_var("_FORGETEST_TEST_VAR");
    }

    #[test]
    fn default_config() {
        let config = ForgetestConfig::default();
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.parallelism, 4);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn parse_provider_config() {
        let toml_str = r#"
[providers.anthropic]
type = "anthropic"
api_key = "sk-test"

[providers.openai]
type = "openai"
api_key = "sk-openai"

[providers.ollama]
type = "ollama"
base_url = "http://localhost:11434"

default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"
"#;
        let config: ForgetestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.providers.len(), 3);
        assert!(matches!(
            config.providers.get("anthropic"),
            Some(ProviderConfig::Anthropic { .. })
        ));
    }
}
