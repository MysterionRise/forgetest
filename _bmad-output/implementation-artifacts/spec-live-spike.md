---
title: 'Live spike: OpenRouter fact-finding and golden stream fixtures'
type: 'chore'
created: '2026-10-18'
status: 'ready-for-dev'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/forgetest-roadmap-2026-10.md'
  - '{project-root}/_bmad-output/planning-artifacts/governance-lean-bmm-ruling.md'
  - '{project-root}/docs/governance/decisions/2026-10-18-don-decisions-months-1-2.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** S3, S4 and pilot-r2 depend on facts that only live calls through
OpenRouter can settle:

- which auth path works with `--bare`;
- whether Codex provider overrides work alongside `--ignore-user-config`;
- whether guardrails are enforced on both endpoints;
- whether effort reaches the API;
- whether Codex `workspace-write` works inside our container;
- whether Codex requires a git repo;
- the exact OpenRouter slugs and response ID formats.

The usage mapping also lacks fixtures captured from real CLI output.

**Approach:**
- Run one capped, approved fact-finding spike in throwaway containers, using a
  key the Don creates: USD 2 limit, 48-hour expiry, allowlist of the four study
  models.
- Exercise both CLIs end to end on a tiny canary repository.
- Record each answer with evidence in a findings memo labelled "not evidence".
- Commit redacted golden fixtures for S4.

## Boundaries & Constraints

**Always:**
- Follow ruling R3:
  - this plan is the approved plan, with a per-run cap of USD 2;
  - the key is scoped and expiring, with its spend limit set in the OpenRouter
    dashboard;
  - no subscription credentials;
  - web tools are disabled on every call.
- Each run happens in a throwaway container on the maintainer's machine, which
  counts as the disposable environment.
- The key is read from the host environment variable `FORGETEST_SPIKE_KEY`. It
  is never echoed, logged, committed or pasted into chat.
- Read billed usage with `GET /api/v1/key` before reporting completion.
- Fixtures are redacted: no keys, host paths or account identifiers.

**Ask First:**
- Any spend above USD 2, or any model outside the allowlist apart from the
  single negative test.
- Using `danger-full-access`.
- Publishing any spike output as evidence.
- Any Rust source change. Fixture-consuming tests belong to S4.

**Never:**
- Touch `studies/`.
- Run on shared or long-lived infrastructure.
- Store raw unredacted streams in Git.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Slug discovery | `GET https://openrouter.ai/api/v1/models` (no key) | Exact slugs and prices for the 4 study models recorded | N/A |
| Claude auth | `--bare` with an `apiKeyHelper` in `--settings`; then `ANTHROPIC_AUTH_TOKEN`; with `ANTHROPIC_BASE_URL` set to OpenRouter | First working path recorded with its `result` event | All fail: recorded as a blocker |
| Codex provider | `-c model_provider=openrouter -c model_providers.openrouter={…}` plus `--ignore-user-config` | Works, or fails with a recorded error | Fallbacks tried in plan order |
| Guardrail negative | A request for a model not on the key's allowlist | Rejected (404 or similar) with zero charge, on both endpoints | Unexpected acceptance recorded as a blocker |
| Effort on the wire | `medium` effort through each CLI | Shown via CLI debug logs or OpenRouter activity | Recorded "unverified" |
| Codex sandbox canary | forgetest's agent Docker flags, `--sandbox workspace-write`, a task that edits a file and runs `cargo test` | Pass or fail recorded; failure means a Don decision at the D5 go/no-go | Never silently switch the mode |
| Codex git repo | Workspace without `.git` | Whether `--skip-git-repo-check` is needed | Recorded for S3 |
| Budget | All calls | Billed usage ≤ USD 2, read from `GET /api/v1/key` | Stop at the limit |

</frozen-after-approval>

## Code Map

- `crates/forgetest-agents/src/lib.rs`:
  - 297-342: current Codex and Claude profile arguments, the baseline the spike
    starts from.
  - 596-641: agent Docker flags the canary must reproduce.
  - ~1419-1520: `claude_result_usage` and `codex_turn_usage`. The fixtures
    must exercise these.
- `docker/agents/*.Dockerfile` from S1, when ready. Otherwise use stock
  `node:22` with npm-pinned `@openai/codex@0.163.0` and
  `@anthropic-ai/claude-code@2.1.302`, for the API questions only.
- `eval-suites/rust-agent-v1/pilot.toml` -- not used. The canary is a separate
  scratch crate under `results/spike-2026-10/canary/`.

## Tasks & Acceptance

**Execution:**
- [ ] `results/spike-2026-10/canary/` -- a tiny Rust crate with a failing test
  and a one-line fix prompt.
- [ ] `results/spike-2026-10/run-spike.sh`
  - Runs each scenario in a throwaway container.
  - Reads `FORGETEST_SPIKE_KEY` from the host env.
  - Passes web tools off on every call.
  - Captures raw streams to an untracked, gitignored directory.
- [ ] `crates/forgetest-agents/tests/fixtures/openrouter/` -- redacted golden
  streams:
  - a Claude `result` event including `modelUsage`;
  - Codex `turn.completed` events including `cache_write_input_tokens`.
- [ ] `results/spike-2026-10/FINDINGS.md`
  - One section per I/O row with evidence excerpts.
  - Recommended S3 and S4 adjustments.
  - The D5 go/no-go on sandbox and auth.
  - A run record: plan ID, commit, cap, billed usage, key revoked.
  - Labelled "fact-finding, not evidence".

**Acceptance Criteria:**
- Given the approved key, when the spike completes, then every I/O row has a
  recorded outcome and billed usage is at most USD 2.
- Given the committed fixtures, when a redaction scan runs, then no key, host
  path or account identifier is present.

## Verification

**Commands:**
- `bash results/spike-2026-10/run-spike.sh` -- expected: completes every
  scenario, with streams in the untracked directory.
- `grep -rE "sk-or-|/Users/" crates/forgetest-agents/tests/fixtures/openrouter results/spike-2026-10` -- expected: no matches.
- `bash scripts/verify-bmad.sh` -- expected: pass.
