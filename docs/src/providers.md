# Providers and Agents

`forgetest` has two distinct integration layers.

## Coding Agents

Repository suites use external coding-agent CLIs through `AgentExecutor`.

Built-in profiles:

- `codex`: noninteractive JSONL execution with an exact model.
- `claude`: noninteractive stream-JSON execution with an exact model.
- `generic`: library adapter for an arbitrary external command.
- `scripted`: deterministic offline adapter used by tests and demos.

Check local installations:

```bash
forgetest agents doctor --agents codex/MODEL,claude/MODEL
```

The doctor command reports executable path, version, binary SHA-256, and
whether required credential variables are present. It never prints credential
values.

Model names are supplied explicitly by the operator. Documentation does not
claim a permanently current vendor model list. Benchmark mode freezes the exact
selection in `benchmark.lock.toml` and rejects common moving aliases such as
`default`, `latest`, `sonnet`, `opus`, and `haiku`.

## Legacy Completion Providers

Snippet eval sets support Anthropic, OpenAI-compatible APIs, and Ollama.

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
```

Use exact model IDs appropriate to the provider at run time:

```bash
forgetest run \
  --config ./forgetest.toml \
  --eval-set eval-sets/rust-basics.toml \
  --models anthropic/MODEL,openai/MODEL
```

Every credential reference in a configured provider section must resolve when
the configuration is loaded. Remove or comment out unused provider sections
instead of leaving references to unset credentials.

`forgetest list-models` queries Ollama asynchronously. Static provider entries
are informational and should not be treated as a vendor availability promise.

```bash
forgetest list-models --config ./forgetest.toml --provider ollama
```

A project-local `forgetest.toml` is used only when passed with `--config`.
Without that flag, `forgetest` checks
`~/.config/forgetest/config.toml`.

## Credential References

Only exact documented credential references are interpolated:

- `${OPENAI_API_KEY}`
- `${FORGETEST_OPENAI_KEY}`
- `${ANTHROPIC_API_KEY}`
- `${FORGETEST_ANTHROPIC_KEY}`

Repository task files cannot select provider hosts, images, credentials, or
arbitrary environment interpolation.
