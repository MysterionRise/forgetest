# Rust Agent Pilot Protocol

## Status

Pre-registered on 2026-07-29. External-agent execution has not occurred and
this directory contains no performance claim.

Do not add rates or conclusions without the matching public evidence bundle,
artifact manifest, exact CLI identities, run commit, and manual redaction
review.

## Amendment 2026-10-07

Procedural only. The question, tasks, agents, models, effort, budgets, and
publication gate below are unchanged.

- The run commit must include the host-agent toolchain fix from the
  `Unreleased` changelog section. Before it, development-mode agents ran with
  an isolated home that hid the host rustup state, so they could not run
  `cargo` in task workspaces.
- A smoke run on one non-pilot task precedes the pilot (see
  [Smoke Run](#smoke-run)). Smoke trials are excluded from all pilot and study
  results.
- If either pinned model is no longer served, record the substitution as a
  protocol deviation in `RESULTS.md` instead of silently changing it.

## Question

Can the release-candidate Codex CLI and Claude Code configurations complete one
boundary bug fix, one async coordination fix, and one path-security fix through
the full `forgetest` lifecycle?

This is a six-trial integration pilot, not a ranking or statistically powered
model comparison.

## Frozen Design

```text
3 tasks x 2 agent configurations x 1 trial = 6 scheduled trials
```

| Agent | Exact model | Effort |
|---|---|---|
| Codex CLI | `gpt-5.6-sol` | CLI default |
| Claude Code | `claude-sonnet-5` | CLI default |

The model strings are pinned release identifiers, not `latest`, `sonnet`, or
other moving aliases. The pilot suite is
`eval-suites/rust-agent-v1/pilot.toml`:

- `range-boundary`: compact correctness bug.
- `once-notify`: async/concurrency behavior.
- `safe-path`: path traversal robustness.

## Execution Policy

- Profile: `development`.
- Agent environment: trusted host process with isolated home and explicit
  credential allowlist.
- Verifier: Docker, non-root, read-only root, no network, no capabilities,
  `no-new-privileges`, tmpfs, and resource limits.
- Trials: one per task and agent.
- Parallelism: one.
- Agent timeout: 900 seconds per trial.
- Agent output cap: 4 MiB.
- Agent-reported token cap: 200,000 per trial.
- Agent-reported cost cap: USD 10 per trial.
- Retries: zero.

These usage caps are evaluated from vendor-reported events and are not an
independent billing control.

## Smoke Run

Run one trial per agent on `eval-suites/rust-agent-v1/smoke.toml`, which holds
only `retry-policy`, a task outside the pilot. Use the same policy as the pilot
and a fresh output path outside `studies/`:

```bash
forgetest run \
  --suite eval-suites/rust-agent-v1/smoke.toml \
  --agents codex/gpt-5.6-sol,claude/claude-sonnet-5 \
  --trials 1 \
  --profile development \
  --runner docker \
  --parallelism 1 \
  --agent-timeout-secs 900 \
  --max-agent-output-bytes 4194304 \
  --max-agent-tokens 200000 \
  --max-agent-cost-usd 10 \
  --agent-retries 0 \
  --output /tmp/forgetest-pilot-smoke \
  --format all
```

Proceed to the pilot only when:

- Both trials end as `passed` or `failed`, not `agent_error`,
  `environment_error`, or `timeout`.
- Each `raw/trials/<trial-id>/trace.jsonl` shows the agent running `cargo`
  successfully in its workspace.
- Reported usage and, where the vendor reports it, the resolved model look
  plausible.

Keep the raw smoke bundle private. It is the reference sample of real CLI
output for adapter and usage-parser tests, and it is not published.

## Commands

The isolated host adapters require `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`.
Interactive CLI login state is deliberately not copied into the trial home.

```bash
forgetest agents doctor \
  --agents codex/gpt-5.6-sol,claude/claude-sonnet-5

forgetest validate \
  --suite eval-suites/rust-agent-v1/pilot.toml \
  --calibrate

docker build \
  -f docker/forgetest-runner-rust.Dockerfile \
  -t forgetest-runner-rust:0.1.0 .

forgetest run \
  --suite eval-suites/rust-agent-v1/pilot.toml \
  --agents codex/gpt-5.6-sol,claude/claude-sonnet-5 \
  --trials 1 \
  --profile development \
  --runner docker \
  --parallelism 1 \
  --agent-timeout-secs 900 \
  --max-agent-output-bytes 4194304 \
  --max-agent-tokens 200000 \
  --max-agent-cost-usd 10 \
  --agent-retries 0 \
  --output studies/rust-agent-pilot/run \
  --format all
```

## Publication Gate

- [ ] Record clean run commit, OS, architecture, Docker version, and run date.
- [ ] Record the smoke run date and outcome, and confirm it passed its checks.
- [ ] Record both CLI versions and executable SHA-256 values.
- [ ] Confirm all six scheduled trial records exist.
- [ ] Preserve failures, timeouts, and infrastructure errors.
- [ ] Review raw evidence privately.
- [ ] Generate and manually inspect the redacted public bundle.
- [ ] Verify every artifact manifest entry independently.
- [ ] Complete `RESULTS.template.md` and rename it to `RESULTS.md`.
- [ ] Link the immutable commit and CI/release evidence.

The full benchmark remains the separately pre-registered 72-trial study under
`studies/rust-agent-v1`.
