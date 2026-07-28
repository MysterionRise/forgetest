//! The `forgetest list-models` command.

use std::path::PathBuf;

use anyhow::Result;

use forgetest_providers::config::ProviderConfig;
use forgetest_providers::create_provider;
use forgetest_providers::ollama::OllamaProvider;

pub async fn execute(provider_filter: Option<String>, config_path: Option<PathBuf>) -> Result<()> {
    let config = forgetest_providers::config::load_config_from(config_path.as_deref())?;

    let mut found_any = false;

    for (name, provider_config) in &config.providers {
        if let Some(filter) = &provider_filter {
            if name != filter {
                continue;
            }
        }

        let live_catalog = matches!(provider_config, ProviderConfig::Ollama { .. });
        let models = match provider_config {
            ProviderConfig::Ollama { base_url } => {
                let provider = OllamaProvider::new(base_url);
                match provider.list_models_async().await {
                    Ok(models) => models,
                    Err(e) => {
                        eprintln!("Provider: {name}");
                        eprintln!("  Could not list Ollama models: {e}");
                        found_any = true;
                        continue;
                    }
                }
            }
            _ => {
                let provider = create_provider(name, provider_config)?;
                provider.available_models()
            }
        };

        if !models.is_empty() {
            found_any = true;
            println!("Provider: {name}");
            if !live_catalog {
                println!(
                    "  Catalog: built-in informational snapshot; verify current availability and pricing"
                );
            }
            for model in &models {
                println!(
                    "  {} — {} ({}K context, ${:.4}/{:.4} per 1K tokens)",
                    model.id,
                    model.name,
                    model.max_context / 1000,
                    model.cost_per_1k_input,
                    model.cost_per_1k_output,
                );
            }
            println!();
        }
    }

    if !found_any {
        println!("No providers configured. Run `forgetest init` to create a config file.");
    }

    Ok(())
}
