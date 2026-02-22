# Writing Eval Cases

Eval cases are defined in TOML files. Each file contains an `[eval_set]` header and one or more `[[cases]]` entries.

## Eval Set Header

```toml
[eval_set]
id = "my-evals"                   # Unique identifier (required)
name = "My Eval Set"              # Human-readable name (required)
description = "Custom eval cases" # Optional description
default_language = "rust"         # Default language for all cases
default_timeout_secs = 60         # Default timeout per case
```

## Case Definition

```toml
[[cases]]
id = "fibonacci"                  # Unique ID within the set (required)
name = "Fibonacci function"       # Human-readable name (required)
description = "Iterative fib"     # Optional description
prompt = """                      # The prompt sent to the LLM (required)
Write a Rust function `fn fibonacci(n: u64) -> u64` that returns
the nth Fibonacci number using an iterative approach.
"""
tags = ["algorithms", "basics"]   # Tags for filtering
timeout_secs = 120                # Override default timeout
max_tokens = 4096                 # Override max tokens for generation
```

## Expectations

Each case has an `[cases.expectations]` section:

```toml
[cases.expectations]
should_compile = true             # Must the code compile? (default: true)
should_pass_tests = true          # Must the tests pass? (default: false)
test_file = """                   # Test code appended to generated code
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fib() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(10), 55);
    }
}
"""
expected_functions = ["fibonacci"] # Functions that must be defined
expected_types = []                # Types/structs that must be defined
```

## Writing Good Test Files

The `test_file` is appended to the generated source code in `lib.rs`. Use `use super::*;` to import the generated functions.

### Tips

1. **Write at least 3 tests per case** — cover normal cases, edge cases, and boundary conditions.
2. **Use clear assertion messages** — when tests fail, the output should help diagnose the issue.
3. **Test both success and failure modes** — if the function should return `None` for invalid input, test that.
4. **Keep tests independent** — each test should be self-contained.

### Example: Comprehensive test file

```toml
test_file = """
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {
        assert_eq!(fibonacci(10), 55);
    }

    #[test]
    fn test_base_cases() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
    }

    #[test]
    fn test_larger_input() {
        assert_eq!(fibonacci(30), 832040);
    }
}
"""
```

## Tag-based Filtering

Use tags to organize and filter cases:

```bash
# Only run cases tagged "algorithms"
forgetest run --eval-set eval-sets/rust-basics.toml --filter algorithms

# Run multiple tags
forgetest run --eval-set eval-sets/rust-basics.toml --filter "algorithms,strings"
```

## Validating Eval Sets

Always validate your eval sets before running:

```bash
forgetest validate --eval-set my-evals.toml
```

This checks for:

- Valid TOML syntax
- Required fields (`id`, `name`, `prompt`)
- Duplicate case IDs
- `should_pass_tests = true` without a `test_file`
- Empty prompts

## Organizing Eval Sets

You can pass a directory to `--eval-set` to run all `.toml` files in it:

```bash
forgetest run --eval-set eval-sets/
forgetest validate --eval-set eval-sets/
```

Group related cases into separate files by topic, difficulty, or feature area.
