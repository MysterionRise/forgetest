# Getting Started

## Build

The workspace pins Rust 1.92.0:

```bash
cargo build --workspace --locked
cargo install --path crates/forgetest-cli --locked
forgetest --version
```

## First Repository Trial

Run the deterministic no-key repository demo:

```bash
forgetest demo \
  --mode repository \
  --runner local \
  --output ./forgetest-results \
  --format all
```

This uses a scripted agent, but the remaining lifecycle is the normal one:

1. Materialize a clean visible workspace.
2. Stream a normalized agent trace.
3. Capture the patch independently of Git.
4. Rebuild a clean verification workspace.
5. Overlay a hidden grader.
6. Execute the required Rust checks.
7. Write private and redacted evidence bundles.

Inspect:

- `forgetest-results/raw/report.html` for private evidence.
- `forgetest-results/public/report.html` for publication-safe evidence.
- Each `artifact-manifest.json` for file SHA-256 values.

The scripted result is offline workflow evidence, not a model benchmark.
Use a new or empty output directory for each repository run.

## Validate the Corpus

Load the strict 12-task suite:

```bash
forgetest validate --suite eval-suites/rust-agent-v1/suite.toml
```

Run trusted calibration:

```bash
forgetest validate \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --calibrate
```

Calibration proves that each visible workspace fails its fail-to-pass check and
that each reference patch passes all required checks. It executes suite
commands on the host and is only for reviewed suites.

## Docker Verification

```bash
docker build \
  -f docker/forgetest-runner-rust.Dockerfile \
  -t forgetest-runner-rust:0.1.0 .

forgetest demo \
  --mode repository \
  --runner docker \
  --output ./forgetest-docker-results \
  --format all
```

The agent remains deterministic and offline. The grader runs in the hardened,
network-disabled Docker verifier.

## External Agents

Check local CLI availability without printing credentials:

```bash
forgetest agents doctor --agents codex/MODEL,claude/MODEL
```

Run one trusted development trial per task:

```bash
forgetest run \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --agents codex/MODEL,claude/MODEL \
  --trials 1 \
  --profile development \
  --runner docker \
  --output ./agent-results \
  --format all
```

Use benchmark mode only after creating an immutable lock. See
[Advanced Usage](./advanced.md).

## Legacy Snippet Demo

The original provider-to-snippet loop remains available:

```bash
forgetest demo \
  --runner local \
  --output ./snippet-results \
  --format all
```

Configure providers only when you need paid or local model calls. No provider
configuration is needed for either demo.
