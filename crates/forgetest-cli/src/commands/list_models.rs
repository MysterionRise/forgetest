//! The `forgetest list-models` command.

use std::path::PathBuf;

use anyhow::Result;

use forgetest_providers::create_provider;

pub fn execute(provider_filter: Option<String>, config_path: Option<PathBuf>) -> Result<()> {
    let config = forgetest_providers::config::load_config_from(config_path.as_deref())?;

    let mut found_any = false;

    for (name, provider_config) in &config.providers {
        if let Some(filter) = &provider_filter {
            if name != filter {
                continue;
            }
        }

        let provider = create_provider(name, provider_config)?;
        let models = provider.available_models();

        if !models.is_empty() {
            found_any = true;
            println!("Provider: {name}");
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
