# Changelog

All notable changes to `forgetest` are documented here.

## Unreleased

### Security

- Upgraded transitive `h2` to 0.4.20 (RUSTSEC-2026-0258) and `rustls` to
  0.23.45 with `rustls-webpki` 0.103.15 (RUSTSEC-2026-0285), and replaced the
  yanked `chacha20` 0.10.1 with 0.10.2.

### Fixed

- Host development agents now inherit the host Rust toolchain location
  (`RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTDOC`) and TLS certificate
  variables. Previously the isolated agent home left rustup without a default
  toolchain, so agents could not run `cargo` in task workspaces. `CARGO_HOME`
  still resolves under the isolated per-trial home. The local grader shares
  the same toolchain discovery.

### Changed

- CI also runs weekly and on manual dispatch, so new advisories fail on
  `master` instead of on the next unrelated pull request.
- Dependabot groups cargo minor/patch updates and GitHub Actions updates into
  one pull request each, so stale updates can no longer exhaust its open
  pull-request limit.

## 0.1.0 - 2026-07-29

First public proof-of-concept release.

### Added

- Strict repository-suite schema and a calibrated 12-task Rust corpus.
- Codex CLI, Claude Code, generic command, and deterministic scripted agents.
- Independent patch capture and clean hidden-grader verification.
- Hardened, network-disabled Docker verifier plus trusted local development mode.
- Typed bounded JSONL traces and explicit timeout, output, retry, token, and
  reported-cost budgets.
- Report schema v2, redacted public bundles, artifact manifests, HTML, JSON,
  and deterministic SARIF.
- Exact benchmark lock and credential-safe agent diagnostics.
- Backward-compatible snippet evaluation and report loading.
- No-key snippet and repository demos, CI proof jobs, release provenance, and a
  browser-ready evidence site.
- Fail-closed repository demos and system-temporary trial workspaces, so output
  paths inside another Cargo workspace cannot invalidate verification.
- Docker verifier preflight before external-agent execution, publication-safe
  path scanning, and resumable crates.io publishing.
- Rustls-backed provider HTTP clients and an ARM64 Linux portability gate.

### Boundaries

- Rust is the only fully supported language.
- Deterministic demo reports are workflow evidence, not model benchmarks.
- The six-trial pilot and 72-trial public study are pre-registered but
  unexecuted.
- Hardened Docker execution is not presented as safe for hostile repositories.
- `v0.1.0` is available as an attested GitHub release; its credential-gated
  crates.io channel was not enabled.
