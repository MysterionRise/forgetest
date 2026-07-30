# Getting Started

## Install or Run from Source

The workspace pins Rust 1.92.0:

```bash
cargo install --path crates/forgetest-cli --locked
forgetest --version
```

Release archives are also available from the
[GitHub releases page](https://github.com/MysterionRise/forgetest/releases).
All examples below use the installed binary. From a source checkout, replace
`forgetest` with `cargo run --locked --bin forgetest --`.

Local verification requires `cargo` and `rustc`; snippet evaluation also
requires Clippy. Docker verification requires Docker Engine.

Commands that refer to `eval-suites/`, `eval-sets/`, or `docker/` assume the
current directory is the root of a source checkout. The bundled demos do not
need those paths.

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

The key output layout is:

```text
forgetest-results/
  raw/
    report.html
    report.json
    artifact-manifest.json
    trials/<trial-id>/changes.patch
    trials/<trial-id>/trace.jsonl
  public/
    report.html
    report.json
    artifact-manifest.json
```

The command exits with status 0 only when the scripted patch passes the hidden
grader. Start with `public/report.html`; use `raw/` only for private debugging.

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

Replace each `MODEL` with an exact identifier accepted by that installed agent
CLI. Benchmark locks reject moving aliases.

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
  --mode snippet \
  --runner local \
  --output ./snippet-results \
  --format all
```

Configure providers only when you need paid or local model calls. No provider
configuration is needed for either demo.

## Create a Snippet Eval Set

`forgetest init` creates a strict starter configuration and two-case snippet
eval set in the current directory:

```bash
mkdir my-forgetest-project
cd my-forgetest-project
forgetest init
forgetest validate --eval-set eval-sets/example.toml
```

Edit `forgetest.toml` to select an exact model and keep only providers you
intend to configure. Set the referenced credential environment variable, then
run:

```bash
forgetest run \
  --config ./forgetest.toml \
  --eval-set ./eval-sets/example.toml \
  --models anthropic/MODEL \
  --output ./snippet-results \
  --format all
```

A project-local `forgetest.toml` is not loaded implicitly. Pass `--config`, or
install the configuration as `~/.config/forgetest/config.toml`.
