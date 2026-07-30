//! The `forgetest init` command.

use anyhow::Result;

pub fn execute() -> Result<()> {
    // Create forgetest.toml
    if std::path::Path::new("forgetest.toml").exists() {
        println!("forgetest.toml already exists, skipping.");
    } else {
        std::fs::write("forgetest.toml", SAMPLE_CONFIG)?;
        println!("Created forgetest.toml");
    }

    // Create example eval set
    std::fs::create_dir_all("eval-sets")?;
    let example_path = std::path::Path::new("eval-sets/example.toml");
    if example_path.exists() {
        println!("eval-sets/example.toml already exists, skipping.");
    } else {
        std::fs::write(example_path, EXAMPLE_EVAL_SET)?;
        println!("Created eval-sets/example.toml");
    }

    println!("\nNext steps:");
    println!("  1. Select an exact model ID and configure the active provider");
    println!("  2. Run: forgetest validate --eval-set eval-sets/example.toml");
    println!("  3. Run: forgetest run --config forgetest.toml --eval-set eval-sets/example.toml");

    Ok(())
}

const SAMPLE_CONFIG: &str = r#"# forgetest configuration

default_provider = "anthropic"
default_model = "replace-with-provider-model-id"
default_temperature = 0.0
parallelism = 4

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

# Configure one active provider at a time unless every credential reference is
# available. To use OpenAI or Ollama instead, change default_provider and
# default_model above, then replace the Anthropic section:
#
# [providers.openai]
# type = "openai"
# api_key = "${OPENAI_API_KEY}"
#
# [providers.ollama]
# type = "ollama"
# base_url = "http://localhost:11434"

[runner]
type = "local"
docker_image = "forgetest-runner-rust:0.1.0"
memory = "512m"
cpus = 1.0
pids_limit = 128
network = "none"
"#;

const EXAMPLE_EVAL_SET: &str = r#"[eval_set]
id = "example"
name = "Example Eval Set"
description = "A simple example eval set to get started"
default_language = "rust"
default_timeout_secs = 60

[[cases]]
id = "add_function"
name = "Add function"
description = "Write a simple add function"
prompt = """
Write a Rust function `fn add(a: i32, b: i32) -> i32` that returns the sum of a and b.
"""
tags = ["basics"]

[cases.expectations]
should_compile = true
should_pass_tests = true
test_file = """
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }
}
"""
expected_functions = ["add"]

[[cases]]
id = "reverse_string"
name = "Reverse string"
description = "Write a function to reverse a string"
prompt = """
Write a Rust function `fn reverse_string(s: &str) -> String` that returns the reversed string.
"""
tags = ["strings", "basics"]

[cases.expectations]
should_compile = true
should_pass_tests = true
test_file = """
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_reverse() {
        assert_eq!(reverse_string("hello"), "olleh");
        assert_eq!(reverse_string(""), "");
        assert_eq!(reverse_string("a"), "a");
    }
}
"""
expected_functions = ["reverse_string"]
"#;

#[cfg(test)]
mod tests {
    use forgetest_providers::config::ForgetestConfig;

    use super::SAMPLE_CONFIG;

    #[test]
    fn generated_config_is_strictly_parseable() {
        let config = toml::from_str::<ForgetestConfig>(SAMPLE_CONFIG).unwrap();
        assert_eq!(
            config.default_model,
            forgetest_providers::config::UNCONFIGURED_MODEL_ID
        );
        assert_eq!(config.providers.len(), 1);
        assert!(config.providers.contains_key("anthropic"));
    }
}
