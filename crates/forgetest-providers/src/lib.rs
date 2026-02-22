//! forgetest-providers — LLM provider integrations.
//!
//! Implements the `LlmProvider` trait for Anthropic, OpenAI, and Ollama,
//! allowing forgetest to generate code from multiple LLM backends.

pub mod anthropic;
pub mod config;
pub mod error;
pub mod mock;
pub mod ollama;
pub mod openai;

pub use config::{create_provider, load_config, ForgetestConfig, ProviderConfig};
pub use error::ProviderError;
