# Benchmark Operations

## Development and Benchmark Profiles

`development` is for trusted iteration:

- Installed host agent CLI.
- Explicit environment allowlist and isolated home.
- Local or Docker verifier.
- Model and CLI identity recorded, but no immutable image lock required.

`benchmark` is for publishable studies:

- Agent CLI in an ephemeral outer container.
- Independent Docker verifier.
- Full immutable image digests.
- Exact CLI version and executable SHA-256 preflight.
- Exact model, effort, suite, and complete execution-policy lock.
- Requested-model, command-profile, binary, image, suite, and policy drift
  rejected.

## Build Agent Images

v1 deliberately does not ship vendor credentials or a universal agent image.
Build one image per adapter that:

- Contains the exact `codex` or `claude` executable.
- Runs correctly as an arbitrary non-root UID.
- Uses environment credentials at runtime.
- Does not require the host home or Docker socket.
- Supports `--version` without network or credentials.

Push the images and retain full registry digests.
See the exact
[agent image contract](https://github.com/MysterionRise/forgetest/blob/master/docker/agents/README.md).

## Create a Benchmark Lock

```bash
forgetest agents lock \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --agent codex/MODEL=registry.example/codex@sha256:DIGEST \
  --agent claude/MODEL=registry.example/claude@sha256:DIGEST \
  --effort codex=high \
  --effort claude=high \
  --verifier-image registry.example/forgetest-runner-rust@sha256:DIGEST \
  --trials 3 \
  --parallelism 2 \
  --agent-timeout-secs 900 \
  --max-agent-output-bytes 4194304 \
  --max-agent-tokens 200000 \
  --max-agent-cost-usd 10 \
  --max-run-cost-usd 30 \
  --pricing pricing.toml \
  --agent-retries 0 \
  --output benchmark.lock.toml
```

Codex reports no cost, so its cost caps need a `pricing.toml` entry for the
exact model; see [Pricing Tables](#pricing-tables).

The lock command runs a credential-free, network-disabled preflight inside each
agent image and records the observed executable hash and version.
It rejects common moving aliases such as `default`, `latest`, `sonnet`, `opus`,
and `haiku`; use the exact provider model identifier intended for the study.
The harness cannot independently detect provider-side model substitution unless
the vendor reports the resolved model in its event stream.

Re-audit the locked agent and verifier images later:

```bash
forgetest agents doctor --benchmark-lock benchmark.lock.toml
```

## Run the Study

```bash
forgetest run \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --agents codex,claude \
  --trials 3 \
  --profile benchmark \
  --benchmark-lock benchmark.lock.toml \
  --parallelism 2 \
  --agent-timeout-secs 900 \
  --max-agent-tokens 200000 \
  --max-agent-cost-usd 10 \
  --max-run-cost-usd 30 \
  --pricing pricing.toml \
  --output ./runs/2026-rc1 \
  --format all
```

The benchmark lock must match the complete effective policy. Changing
parallelism, a budget, the run cost cap, any price in the pricing table, retry
count, image, task content, or suite membership requires a new lock.

Use a new or empty output directory for every run. Repository commands reject
non-empty evidence directories so stale files cannot enter a new inventory.

## Failure Accounting

Every scheduled trial is retained:

| Status | Meaning |
|---|---|
| `passed` | Every required verifier check passed |
| `failed` | Agent completed, patch applied, required check failed |
| `agent_error` | Adapter/process failed or returned non-zero, or a budget was exceeded or unverifiable |
| `environment_error` | Materialization or execution environment failed |
| `grader_error` | Verifier infrastructure failed |
| `timeout` | Agent exhausted its wall-time budget |
| `cancelled` | Trial never started because the run cost breaker tripped |

Agent errors and timeouts count against observed agent reliability.
Environment/grader errors are also broken out as infrastructure errors.

## Retries and Budgets

Retries apply only after agent errors or non-zero exits. Before a retry,
`forgetest` restores the pristine visible workspace. Agent time, reported
tokens, and reported cost are cumulative across attempts.

Output-byte, 10,000-event, and timeout limits terminate Unix process groups and
force-remove containers. Windows host development mode terminates the direct
child.
Token and cost caps are checked after the agent exits; they classify a trial
but do not stop a running agent. A token cap counts uncached input, cache-read,
cache-write, and output tokens.
Token/cost values depend on normalized vendor events and are not independent
billing records.

## Usage and Cost Sources

Each trial records normalized usage:

| Field | Meaning |
|---|---|
| `input_tokens` | Uncached input tokens |
| `cached_tokens` | Cache-read input tokens |
| `cache_write_tokens` | Cache-write input tokens |
| `output_tokens` | Output tokens, including reasoning |
| `reasoning_tokens` | Informational part of `output_tokens`, never priced separately |
| `estimated_cost_usd` | Trial cost in USD |
| `cost_source` | `vendor`, `pricing-table`, or `unavailable` |

Claude Code usage comes from the final `result` event, including its
`total_cost_usd`. Codex usage comes from the last `turn.completed` event. Codex
counts cached input inside `input_tokens`, so `forgetest` records
`input_tokens` minus `cached_input_tokens` as uncached input; an event whose
cached input exceeds its input is treated as malformed and records no usage.
Token counts are recorded only when a stream reports both input and output
tokens. This mapping follows vendor documentation and has not yet been
confirmed against recorded live streams.

Cost follows a fixed precedence:

1. `vendor`: the agent stream reported a cost.
2. `pricing-table`: the stream reported tokens, and the policy pricing table
   has an entry for the exact model ID.
3. `unavailable`: neither applies, including when the stream reported no
   usage at all. The cost is then excluded from totals instead of being counted
   as zero.

Aggregate `total_cost_usd` excludes unavailable costs, and
`cost_unavailable_trials` counts them. HTML reports show the count next to the
cost, for example `$0.1200 (2 unavailable)`. Reports written before these fields
existed omit them and still verify unchanged.

### Pricing Tables

`forgetest` ships no prices and never fetches them. To price an agent that
reports tokens without a cost, copy the vendor's published prices into a
`pricing.toml`:

```toml
schema_version = 1
retrieved_on = "2026-10-15"

[[models]]
model = "EXACT-MODEL-ID"
input_usd_per_mtok = 1.25
cache_read_usd_per_mtok = 0.125
cache_write_usd_per_mtok = 0.0
output_usd_per_mtok = 10.0
```

The numbers above are placeholders, not vendor prices. Prices are USD per
million tokens, and every field is required. Unknown fields, negative or
non-finite prices, duplicate model IDs, and a `retrieved_on` date later than
today are rejected before any trial runs, and the error never repeats the
file's host path. Model IDs match exactly: aliases, prefixes, and case variants
are never priced. Entries are sorted by model ID when the file is read, so
reordering a file does not change the policy digest.

Pass the file with `--pricing` to `run` and `agents lock`. The table is stored
inline in the execution policy rather than as a path, so the policy digest
covers every price and redacted public reports still verify and show the table.

## Run Cost Cap

`--max-run-cost-usd` adds a run-level cost breaker. Before starting each trial,
`forgetest` sums the recorded cost of finished trials. Once that sum reaches the
cap, or once any finished trial that ran an agent has no known cost, every trial
that has not started is recorded as `cancelled` with zero agent attempts and the
message `run cost cap reached` or `run cost unverifiable`.

Trials start round-robin: every agent and task gets trial 1 before any gets
trial 2. A run the breaker truncates therefore stays balanced across agents and
tasks, and the report keeps its usual agent, task, and trial order. Cancelled
trials stay in the `scheduled` count as non-passes, so a truncated run lowers
observed rates. Treat it as incomplete evidence, not as a finished study.

The breaker never stops a running agent. With parallelism `p`, at most `p - 1`
other trials are still running when the cap is reached. Each can add up to the
per-trial cost cap, so in-flight trials can exceed the run cap by at most
`(p - 1)` times `--max-agent-cost-usd`, beyond the cost of the trial that reached
it. That bound assumes every trial stays within its per-trial cap, which is
itself checked only after the agent exits. Without a per-trial cost cap the
overshoot is unbounded, and `run` and `agents lock` warn about it.
Provider-console spend limits remain the hard stop.

## Cap Enforceability

The `benchmark` profile fails closed on budgets:

- `run` and `agents lock` reject a per-trial or run cost cap for an agent that
  reports no vendor cost and has no pricing entry for its exact model, and name
  the agent and model. Codex reports no cost, so a Codex cost cap needs a
  pricing entry. The scripted agent reports no usage and cannot enforce any cap.
- A trial whose agent exits without the usage a configured cap needs is
  recorded as `agent_error` with "budget unverifiable" instead of being graded.
- A trial does not retry after an attempt whose usage the run cap needs is
  unknown, because the next attempt could exceed the cap unseen.

The `development` profile prints a warning for an unenforceable cap, runs the
trial, and records its cost as `unavailable`. With `--max-run-cost-usd`, the
first such trial to finish trips the breaker, which cancels every trial not yet
started. In both profiles, a trial with a per-trial token or cost cap does not
retry after an attempt whose usage that cap needs is unknown; it ends as
`agent_error` with "budget unverifiable" instead of treating the unknown usage
as zero.

## Upgrading Existing Policies

These changes keep existing reports, benchmark locks, and policy digests valid,
but some trials can be classified differently under an unchanged digest:

- `--max-agent-tokens` now counts uncached input, cache-read, cache-write, and
  output tokens. It previously counted only input and output, so an existing
  token cap can bind sooner.
- Claude Code usage now comes from the final `result` event, which includes
  cache tokens and `total_cost_usd`.
- Usage records gain `cost_source`; missing usage is `unavailable` instead of
  zero.
- `--pricing` and `--max-run-cost-usd` are new. Using either changes the policy
  digest and needs a new lock.

## Compare Runs

```bash
forgetest compare \
  --baseline ./runs/baseline/raw/report.json \
  --current ./runs/candidate/raw/report.json \
  --threshold 0.05 \
  --fail-on-regression
```

Schema v2 regression gates require equal suite digest, all task digests, and
execution-policy digest:

```bash
forgetest compare \
  --baseline old.json \
  --current changed-policy.json \
  --allow-incomparable
```

The second command is explicitly non-gating.

## Public Evidence

Repository runs write both bundles automatically. To re-redact an existing raw
report:

```bash
forgetest redact \
  --input ./runs/2026-rc1/raw/report.json \
  --output ./runs/2026-rc1/public-review \
  --format all
```

Review public output manually before release. The sanitizer is deterministic
for its inputs but cannot recognize every possible secret encoding.

## Harbor Subset

```bash
forgetest harbor export \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --output ./harbor-export \
  --base-image registry.example/verifier@sha256:DIGEST
```

Import accepts only tasks carrying the forgetest bridge marker. The bridge
preserves named checks but does not claim support for general Harbor services,
multi-container environments, or arbitrary setup logic.

## Legacy Programmatic API

The `EvalEngine`, `LlmProvider`, and `CodeRunner` APIs remain available for
snippet evaluation. Internal Rust APIs for repository execution may evolve
before v1; the public CLI and old report deserialization are the compatibility
boundary.
