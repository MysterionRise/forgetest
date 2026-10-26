---
title: 'Live spike: OpenRouter fact-finding and golden stream fixtures'
type: 'chore'
created: '2026-10-18'
status: 'done'
baseline_commit: '249947d0869e1cd282e70ea341a2a56fd404a603'
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
- [x] `results/spike-2026-10/canary/` -- a tiny Rust crate with a failing test
  and a one-line fix prompt.
- [x] `results/spike-2026-10/run-spike.sh`
  - Runs each scenario in a throwaway container.
  - Reads `FORGETEST_SPIKE_KEY` from the host env.
  - Passes web tools off on every call.
  - Captures raw streams to an untracked, gitignored directory.
- [x] `crates/forgetest-agents/tests/fixtures/openrouter/` -- redacted golden
  streams:
  - a Claude `result` event including `modelUsage`;
  - Codex `turn.completed` events including `cache_write_input_tokens`.
- [x] `results/spike-2026-10/FINDINGS.md`
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

## Spec Change Log

- **2026-10-22, Don decisions on the spike key.**
  - Triggering findings: the Don supplied a key in the chat channel instead
    of the host environment; `GET /api/v1/key` reported a USD 5 limit, an
    expiry 30 days out and usage 0; the Don stated that the key has no model
    or provider allowlist.
  - Amended conditions (Don: "it's just 5 usd limit, approved"; "Approve;
    revoke after run"; "No allowlists"):
    - the key may carry a limit of up to USD 5; the run's own spend cap stays
      USD 2 and the script enforces it;
    - the 48-hour expiry is replaced by revoking the key immediately after
      the run, which is mandatory because the key is in the session
      transcript;
    - the guardrail negative row is recorded as not testable with this key;
      allowlist verification moves to the S5 per-arm keys.
  - Known-bad states avoided: spending above the approved USD 2 run cap; a
    long-lived key left active after exposure in the transcript; a guardrail
    "blocker" that only reflects an unconfigured allowlist.
  - KEEP: the key never appears in Git, logs, BMAD output or a command's
    arguments; web tools stay off on every call; throwaway containers only.
- **2026-10-26, Don decisions at the D5 go/no-go.**
  - Triggering findings: the live canary showed that Codex `workspace-write`
    cannot run a command or write a file under the agent container flags;
    the run spent USD 0.005034875 and the key was not yet revoked.
  - Decided (Don: "danger-full-access in container (Recommended)"; "Keep
    using it for now, no issues"):
    - D5 sandbox: Codex runs with `danger-full-access` inside the hardened
      agent container, which is its only sandbox, as Claude Code's is with
      `bypassPermissions`. S3 sets it in the profile; pilot-r2 pre-registers
      and discloses it;
    - the key stays active at the Don's choice instead of being revoked
      after the run; the run record says so. It is not used for S5
      published runs, which need per-arm keys with allowlists.
  - KEEP: no key material in Git, logs or BMAD output.

## Verification

**Commands:**
- `bash results/spike-2026-10/run-spike.sh` -- expected: completes every
  scenario, with streams in the untracked directory.
- `grep -rE "sk-or-|/Users/" crates/forgetest-agents/tests/fixtures/openrouter results/spike-2026-10` -- expected: no matches.
- `bash scripts/verify-bmad.sh` -- expected: pass.

## Suggested Review Order

**Answers and decisions**

- Entry point: one line per question, including the flipped audit conclusion.
  [`FINDINGS.md:15`](../../results/spike-2026-10/FINDINGS.md#L15)

- D5: sandbox no-go confirmed end to end, Don decision and its consequences.
  [`FINDINGS.md:424`](../../results/spike-2026-10/FINDINGS.md#L424)

- Billed versus stream-priced cost reconciles exactly; settlement lag stated.
  [`FINDINGS.md:282`](../../results/spike-2026-10/FINDINGS.md#L282)

- Inputs S3 and S4 must apply, now tracked in deferred work.
  [`FINDINGS.md:359`](../../results/spike-2026-10/FINDINGS.md#L359)

**Key handling and spend control**

- Strict key gates by default; overrides need a Don decision record.
  [`run-spike.sh:44`](../../results/spike-2026-10/run-spike.sh#L44)

- Preflight fails closed on limit, expiry and an unreadable key.
  [`run-spike.sh:300`](../../results/spike-2026-10/run-spike.sh#L300)

- Run-relative budget check before each spending scenario.
  [`spike_tools.py:293`](../../results/spike-2026-10/spike_tools.py#L293)

- Key reaches containers only through env or tmpfs, never argv or host files.
  [`run-spike.sh:264`](../../results/spike-2026-10/run-spike.sh#L264)

- Settlement polling replaces the single 30-second read.
  [`run-spike.sh:817`](../../results/spike-2026-10/run-spike.sh#L817)

- Guardrail is opt-in, outside the default scenario list.
  [`run-spike.sh:69`](../../results/spike-2026-10/run-spike.sh#L69)

**Canaries and fixtures**

- Codex canary runs the real profile flags and never switches sandbox mode.
  [`run-spike.sh:799`](../../results/spike-2026-10/run-spike.sh#L799)

- Fixtures are staged, checked, then moved; failures leave nothing behind.
  [`run-spike.sh:894`](../../results/spike-2026-10/run-spike.sh#L894)

- Redactor keeps pairings via stable placeholders; check-fixture enforces them.
  [`spike_tools.py:599`](../../results/spike-2026-10/spike_tools.py#L599)

- Fixture provenance, redaction rules and the not-evidence label.
  [`README.md:1`](../../crates/forgetest-agents/tests/fixtures/openrouter/README.md#L1)

**Governance**

- Don decisions on the key: USD 5 limit, expiry, no allowlists.
  [`2026-10-22-don-s2-key.md:1`](../../docs/governance/decisions/2026-10-22-don-s2-key.md#L1)

- D5 decision, key kept active, missed revocation acknowledged.
  [`2026-10-26-don-d5-codex-sandbox-and-key.md:1`](../../docs/governance/decisions/2026-10-26-don-d5-codex-sandbox-and-key.md#L1)

**Peripherals**

- Offline tests: budget boundary, preflight gates, redaction, argument guards.
  [`test-spike-tools.sh:39`](../../results/spike-2026-10/test-spike-tools.sh#L39)

- Canary crate: one off-by-one bug and a failing test.
  [`lib.rs:2`](../../results/spike-2026-10/canary/src/lib.rs#L2)
