# Rust Agent v1 Study Protocol

## Status

Protocol ready. External-agent run not yet executed.

Do not add success rates to this directory without the matching benchmark lock,
public evidence bundle, artifact checksums, workflow/run reference, and manual
redaction review.

## Question

For the dated release-candidate configurations selected for Codex CLI and
Claude Code, what fraction of the calibrated `rust-agent-v1` repository tasks
is resolved, how reliable are three independent trials, and what uncertainty,
cost, and infrastructure failure accompanies the observed difference?

This is a comparison of two exact configurations on one controlled corpus. It
is not a general model or vendor leaderboard.

## Design

```text
12 tasks x 2 agent configurations x 3 independent trials = 72 trials
```

Primary endpoint:

- Binary repository resolution: every required fail-to-pass and pass-to-pass
  check succeeds.

Secondary evidence:

- Per-status counts.
- Wilson 95% interval for observed resolution rate.
- Pass@1.
- Pass^3 (all first three trials pass).
- Task-paired agent B minus agent A resolution difference.
- Deterministic 10,000-resample paired bootstrap 95% interval.
- Agent-reported input/output tokens and cost.
- Wall time and infrastructure failure counts.

## Pre-Registration

Before credentials are exposed to a run:

1. Record repository commit and confirm a clean worktree.
2. Run all quality gates.
3. Run `forgetest validate --suite ... --calibrate`.
4. Select exact model IDs and effort values.
5. Build agent and verifier images.
6. Resolve and record complete immutable image digests.
7. Generate `benchmark.lock.toml`.
8. Commit or otherwise time-stamp the protocol and lock.
9. Set trial count, concurrency, time/output/token/cost/retry budgets once.
10. Define cancellation and infrastructure-rerun policy.

No task, grader, prompt, reference patch, model, effort, image, budget, or retry
policy may change after the lock without creating a new study.

## Infrastructure Policy

- Disposable patched Linux worker.
- No host home or Docker socket in agent containers.
- Agent containers may use network for the hosted API.
- Verifier containers use `--network none`.
- Raw traces are access-controlled and retained privately.
- A trial is never deleted because its outcome is inconvenient.
- Infrastructure failures are reported. A replacement run, if allowed by the
  pre-registered policy, receives a new trial index and remains traceable.

## Commands

Use placeholders only until release-candidate selection:

```bash
forgetest agents lock \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --agent codex/MODEL=IMAGE@sha256:DIGEST \
  --agent claude/MODEL=IMAGE@sha256:DIGEST \
  --effort codex=EFFORT \
  --effort claude=EFFORT \
  --verifier-image IMAGE@sha256:DIGEST \
  --trials 3 \
  --output studies/rust-agent-v1/benchmark.lock.toml

forgetest run \
  --suite eval-suites/rust-agent-v1/suite.toml \
  --agents codex,claude \
  --trials 3 \
  --profile benchmark \
  --benchmark-lock studies/rust-agent-v1/benchmark.lock.toml \
  --output studies/rust-agent-v1/run \
  --format all
```

## Publication Checklist

- [ ] All 72 scheduled trials are present or explicitly accounted for.
- [ ] Suite and policy digests match the pre-registered lock.
- [ ] Agent CLI versions, binary hashes, models, effort, and image digests match.
- [ ] Null/reference calibration passed on the study commit.
- [ ] Raw bundle stored privately.
- [ ] Public bundle generated with `forgetest redact`.
- [ ] Public HTML, JSON, SARIF, patches, and traces manually reviewed.
- [ ] `artifact-manifest.json` hashes independently checked.
- [ ] Results template completed without excluding failures.
- [ ] CI/release URL, commit, environment, and study date recorded.
- [ ] Limitations and conflicts/funding stated.

## Interpretation Limits

- Twelve tasks produce wide uncertainty.
- Fixtures are intentionally compact and do not represent all production Rust.
- Four tasks are reduced upstream adaptations, not full repositories.
- Agent APIs and hosted model backends may change even when client-side
  configuration is locked.
- Vendor-reported token and cost usage may differ from invoiced totals.
- One date/configuration comparison does not establish a durable ranking.
