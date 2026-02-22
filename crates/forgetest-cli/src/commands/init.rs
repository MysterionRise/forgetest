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
    println!("  1. Edit forgetest.toml with your API keys");
    println!("  2. Run: forgetest validate --eval-set eval-sets/example.toml");
    println!("  3. Run: forgetest run --eval-set eval-sets/example.toml");

    Ok(())
}

const SAMPLE_CONFIG: &str = r#"# forgetest configuration

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[providers.ollama]
type = "ollama"
base_url = "http://localhost:11434"

default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"
default_temperature = 0.0
parallelism = 4
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
