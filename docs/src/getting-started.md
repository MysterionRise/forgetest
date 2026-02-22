# Getting Started

## Installation

### From source

```bash
cargo install --path crates/forgetest-cli
```

### From crates.io (after publishing)

```bash
cargo install forgetest-cli
```

## Configuration

Run `forgetest init` to create a starter configuration:

```bash
forgetest init
```

This creates two files:

- `forgetest.toml` — provider configuration (API keys, defaults)
- `eval-sets/example.toml` — a simple example eval set

Edit `forgetest.toml` with your API keys:

```toml
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
```

Environment variables in `${VAR_NAME}` syntax are resolved automatically. You can also set:

- `FORGETEST_ANTHROPIC_KEY` — overrides the Anthropic API key
- `FORGETEST_OPENAI_KEY` — overrides the OpenAI API key

## Your First Eval

### 1. Validate the eval set

```bash
forgetest validate --eval-set eval-sets/example.toml
```

### 2. Run the eval

```bash
forgetest run --eval-set eval-sets/example.toml
```

This will:

1. Load the eval cases from the TOML file
2. Send each prompt to the configured LLM
3. Compile the generated code in an isolated sandbox
4. Run the test suite against the compiled code
5. Compute scores and generate a report

### 3. View results

Results are saved as JSON by default in `./forgetest-results/`. Use `--format html` for a self-contained HTML report, or `--format all` for JSON + HTML + SARIF.

## Built-in Eval Sets

forgetest ships with 30 eval cases across three sets:

| Eval Set | Cases | Difficulty | Description |
|----------|-------|------------|-------------|
| `rust-basics.toml` | 15 | Beginner | Fibonacci, palindromes, binary search, CSV parsing, etc. |
| `rust-algorithms.toml` | 10 | Advanced | Dijkstra, LRU cache, trie, A* pathfinding, etc. |
| `rust-async.toml` | 5 | Advanced | Tokio-based async patterns: timeouts, channels, rate limiting |

Run all built-in evals:

```bash
forgetest run --eval-set eval-sets/ --models anthropic/claude-sonnet-4-20250514
```

## Next Steps

- [Writing Eval Cases](./writing-eval-cases.md) — define your own eval cases
- [Providers](./providers.md) — configure LLM providers
- [Scoring](./scoring.md) — understand how scores are computed
- [CI Integration](./ci-integration.md) — run evals in CI pipelines
