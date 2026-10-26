# OpenRouter golden stream fixtures

Redacted agent CLI streams captured by the S2 live spike
(`spec-live-spike`). They are **fact-finding, not evidence**: test inputs for
the S4 usage mapping, not performance, cost or safety claims. See
`results/spike-2026-10/FINDINGS.md`.

Classification: `repository-visible`. Whether this directory ships in the
crates.io package is an open decision, tracked in deferred work.

## Files

| File | Source scenario | What it shows |
|---|---|---|
| `claude-haiku-5.5-canary.jsonl` | `claude-canary` | Successful Claude Code run: assistant events with a `gen-` `request_id` and `provider`, and a `result` with `usage` and `modelUsage` keyed by the OpenRouter slug |
| `claude-haiku-5.5-error-result.jsonl` | `capture` (`claude-spike`), against a local endpoint that answers 400 | Error `result`: `is_error: true`, `api_error_status: 400`, `total_cost_usd: 0`, empty `modelUsage` |
| `codex-gpt-6-luna-canary.jsonl` | `codex-canary` | Codex run with `--sandbox workspace-write` where every command and file write failed; `turn.completed` with `cache_write_input_tokens` |
| `codex-gpt-6-luna-provider-ok.jsonl` | `codex-provider` (`overrides`) | Short successful Codex turn through the `-c` provider override |

## Provenance

- Live run: `results/spike-2026-10/raw/20261022T012610Z` (gitignored, kept on
  the maintainer's machine), 2026-10-22 01:26–01:30 UTC, through OpenRouter.
- CLIs: Codex CLI 0.163.0 and Claude Code 2.1.302, from the S1 image recipes.
- Models: `anthropic/claude-haiku-5.5` and `openai/gpt-6-luna`.
- Produced by `bash results/spike-2026-10/run-spike.sh fixtures <run dir>`,
  with the exact-key check enabled.

## Redaction

`spike_tools.py redact` rewrites each stream:

- session IDs and UUIDs, and `msg_`, `toolu_`, `gen-`, `req_`, `resp_`,
  `call_` and `rs_` IDs, become stable placeholders, so pairings such as a
  tool call and its result are kept;
- host paths become `/redacted-host-path`;
- the output must contain no key material, home path or email address, and
  every ID-like value must be a placeholder (`spike_tools.py check-fixture`).

Redaction re-serialises every event with sorted keys and drops non-JSON
lines, so key order, whitespace and escaping differ from what the CLIs print.
Parsers under test must not depend on byte layout.
