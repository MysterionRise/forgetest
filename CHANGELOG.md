# Changelog

All notable changes to `forgetest` are documented here.

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

### Boundaries

- Rust is the only fully supported language.
- Deterministic demo reports are workflow evidence, not model benchmarks.
- The six-trial pilot and 72-trial public study are pre-registered but
  unexecuted.
- Hardened Docker execution is not presented as safe for hostile repositories.
