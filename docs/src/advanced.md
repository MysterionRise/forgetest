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
  --agent-retries 0 \
  --output benchmark.lock.toml
```

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
  --output ./runs/2026-rc1 \
  --format all
```

The benchmark lock must match the complete effective policy. Changing
parallelism, a budget, retry count, image, task content, or suite membership
requires a new lock.

Use a new or empty output directory for every run. Repository commands reject
non-empty evidence directories so stale files cannot enter a new inventory.

## Failure Accounting

Every scheduled trial is retained:

| Status | Meaning |
|---|---|
| `passed` | Every required verifier check passed |
| `failed` | Agent completed, patch applied, required check failed |
| `agent_error` | Adapter/process failed or returned non-zero |
| `environment_error` | Materialization or execution environment failed |
| `grader_error` | Verifier infrastructure failed |
| `timeout` | Agent exhausted its wall-time budget |
| `cancelled` | Trial was explicitly cancelled |

Agent errors and timeouts count against observed agent reliability.
Environment/grader errors are also broken out as infrastructure errors.

## Retries and Budgets

Retries apply only after agent errors or non-zero exits. Before a retry,
`forgetest` restores the pristine visible workspace. Agent time, reported
tokens, and reported cost are cumulative across attempts.

Output-byte, 10,000-event, and timeout limits terminate Unix process groups and
force-remove containers. Windows host development mode terminates the direct
child.
Token/cost values depend on normalized vendor events and are not independent
billing records.

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
