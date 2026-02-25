# Providers

forgetest supports three LLM providers out of the box. Configure them in `forgetest.toml`.

## Anthropic

```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
```

Supported models:

| Model | ID | Context | Notes |
|-------|-----|---------|-------|
| Claude Sonnet 4 | `claude-sonnet-4-20250514` | 200K | Best balance of quality and cost |
| Claude Haiku 4.5 | `claude-haiku-4-5-20251001` | 200K | Fastest, most cost-effective |

Usage:

```bash
forgetest run --eval-set eval-sets/rust-basics.toml \
  --models anthropic/claude-sonnet-4-20250514
```

## OpenAI

```toml
[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"
```

Optionally set a custom base URL for OpenAI-compatible APIs:

```toml
[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.custom-provider.com"
```

Supported models:

| Model | ID | Context | Notes |
|-------|-----|---------|-------|
| GPT-4.1 | `gpt-4.1` | 1M | Most capable |
| GPT-4.1 Mini | `gpt-4.1-mini` | 1M | Good balance |
| GPT-4.1 Nano | `gpt-4.1-nano` | 1M | Fastest |

## Ollama (Local Models)

```toml
[providers.ollama]
type = "ollama"
base_url = "http://localhost:11434"
```

Ollama runs models locally. Install Ollama and pull a model first:

```bash
ollama pull codellama
ollama pull deepseek-coder
```

Then use it:

```bash
forgetest run --eval-set eval-sets/rust-basics.toml \
  --models ollama/codellama
```

Available models are discovered dynamically via `ollama list`.

### Timeout Note

Local models can be slow. The default Ollama timeout is 300 seconds per request. If you're running larger models, you may need to increase the `timeout_secs` in your eval cases.

## Multiple Providers

Compare models across providers in a single run:

```bash
forgetest run --eval-set eval-sets/rust-basics.toml \
  --models anthropic/claude-sonnet-4-20250514,openai/gpt-4.1,ollama/codellama
```

## Listing Available Models

```bash
# List all configured providers and their models
forgetest list-models

# Filter to a specific provider
forgetest list-models --provider anthropic
```

## Environment Variables

API keys can reference environment variables using `${VAR_NAME}` syntax in `forgetest.toml`. Additionally, these env vars are checked as overrides:

| Variable | Overrides |
|----------|-----------|
| `FORGETEST_ANTHROPIC_KEY` | `providers.anthropic.api_key` |
| `FORGETEST_OPENAI_KEY` | `providers.openai.api_key` |
