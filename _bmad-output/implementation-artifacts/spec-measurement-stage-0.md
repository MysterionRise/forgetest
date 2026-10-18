---
title: 'Measurement stage 0: trustworthy usage, cost and budget enforcement'
type: 'feature'
created: '2026-10-15'
status: 'done'
baseline_commit: 'c3b8f9a01937a963169d07847b860de00ba42d5a'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/forgetest-roadmap-2026-10.md'
  - '{project-root}/ARCHITECTURE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Agent usage and cost are silently wrong:

- Both agents record USD 0.00. Claude's top-level `total_cost_usd` is never
  read, and Codex reports no cost at all.
- Cache and reasoning tokens are dropped.
- Missing usage becomes 0.
- Caps only bound single trials, and nothing checks whether a configured cap
  can actually be enforced.

As a result, no paid study can honestly claim cost or budget compliance.

**Approach:**

- Normalize usage for each CLI stream.
- Price usage without a vendor cost from an operator-supplied `pricing.toml`
  embedded in the execution policy.
- Record `cost_source`.
- Add a run-level cost breaker that is checked between trials.
- Make the benchmark profile reject caps it cannot enforce.

## Boundaries & Constraints

**Always:**
- All schema changes are additive, using `serde(default)` with
  `skip_serializing_if = "Option::is_none"`.
- Existing v2 reports, benchmark locks and policy digests verify unchanged when
  the new options are unused.
- Pricing matches exact model IDs only.
- The benchmark profile fails closed.
- `REPOSITORY_REPORT_SCHEMA_VERSION` stays 2 and the lock `schema_version`
  stays 1.

**Ask First:**
- Renaming or removing any serialized field.
- Bumping the report or lock schema.
- Adding or changing `TrialStatus` variants.
- Regenerating committed `examples/reports`.
- Adding a dependency.
- Live fixtures contradicting the assumptions in Design Notes.

**Never:**
- Ship vendor prices as defaults.
- Fetch prices over the network.
- Kill agents mid-run (the month-2 meter does that).
- Change web-tool flags (deferred).
- Touch `studies/` or the legacy provider pricing.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Claude vendor cost | `result` event with top-level `total_cost_usd` and `usage` cache fields | Tokens normalized; cost from vendor; `cost_source = vendor` | N/A |
| Codex priced | `turn.completed.usage` plus a pricing entry for the exact model | Uncached input = `input_tokens` − `cached_input_tokens`; `cost_source = pricing-table` | N/A |
| Codex unpriced | No pricing entry | `cost_source = unavailable`; cost excluded from totals | Report shows "unavailable" |
| No usage, cap set | Stream has no usage | Benchmark: `agent_error` "budget unverifiable". Development: recorded `unavailable` | Never recorded as 0 |
| Run cap reached | Cumulative cost ≥ `--max-run-cost-usd` | Remaining jobs recorded `cancelled` with 0 attempts; scheduled count unchanged | Message "run cost cap reached" |
| Unverifiable run cost | A finished trial has `unavailable` cost while a run cap is set | Breaker trips the same way | Message "run cost unverifiable" |
| Bad pricing file | Unknown field, negative/NaN price, duplicate model | Rejected before any trial; message has no host path | Exit non-zero |
| Unenforceable benchmark cap | Cost cap with an agent lacking both vendor cost and a pricing entry; or any cap with the scripted adapter | `run` and `agents lock` reject it, naming the agent and model | Development profile warns only |

</frozen-after-approval>

## Code Map

- `crates/forgetest-core/src/agent.rs:36-57,95-124` -- `AgentLimits` (not
  persisted) and `AgentUsage` (`input_tokens`, `output_tokens`,
  `cached_tokens`, `estimated_cost_usd`). It is always built via `default()`.
  `AgentOutcome.usage` is `serde(default)`.
- `crates/forgetest-agents/src/lib.rs`:
  - `usage_from_events` (1295-1328) takes the per-field max over
    `raw["usage"]`, falling back to `raw`, and loses Claude's top-level cost.
  - `classify_event` is at 1264-1283; the budget check at 940-965 runs after
    exit.
  - Profiles: Codex at 250-273, Claude at 275-295, `builtin_profile` at
    318-348.
  - The scripted adapter returns zero usage (481).
  - Lock: `deny_unknown_fields` (25-48); validation at 116-200.
- `crates/forgetest-core/src/repository_report.rs`:
  - `ExecutionPolicyParameters` (283-304); digest at 262-275.
  - `TrialResult.usage` (335).
  - `AgentAggregate.total_cost_usd` (488, sum at 571-574).
  - `validate_integrity` recomputes aggregates exactly (85-152).
- `crates/forgetest-core/src/repository_engine.rs`:
  - Jobs at 127-136; `buffer_unordered` at 138-153; the policy-config match at
    108-121.
  - Per-attempt remaining budget at 254-280; `merge_usage` at 691-696.
  - `Cancelled` is only mapped (463), never produced.
  - Failure paths drop usage at 348, 376 and 426.
- `crates/forgetest-cli/src/main.rs:65-71,272-278` -- flags for `run` and
  `agents lock`.
- `crates/forgetest-cli/src/commands/repository_run.rs`:
  - Policy literal at 51-109; limits at 211-217.
  - Benchmark gating at 183-192; preflight at 230-249 and 303-319.
- `crates/forgetest-cli/src/commands/agents.rs:100-194` -- lock validation and
  policy digest; `demo.rs:215-247` holds the other policy literal.
- `crates/forgetest-report/src/html.rs:197-211` -- Cost column.
  `redaction.rs:170-180` drops keys named `reasoning`, so the new field is
  named `reasoning_tokens`.
- Tests:
  - `crates/forgetest-agents/tests/adapters.rs:484-529`
  - `crates/forgetest-core/tests/repository_report.rs:154-199,278-323`
  - `crates/forgetest-core/tests/repository_engine.rs:250-276`
  - `crates/forgetest-cli/tests/cli_tests.rs:406-474,826-884`
- Docs: `docs/src/advanced.md:51-117`, `README.md:427`.

## Tasks & Acceptance

**Execution:**
- [x] `crates/forgetest-core/src/agent.rs`
  - Add `cache_write_tokens` and `reasoning_tokens` (`u64`, default 0).
  - Add `cost_source: Option<CostSource>` (`vendor`, `pricing-table`,
    `unavailable`; kebab-case).
  - Add a `require_enforceable_usage: bool` limit.
- [x] `crates/forgetest-core/src/pricing.rs` (new, registered in `lib.rs`)
  - Strict `PricingTable`: `schema_version = 1`, `retrieved_on`, and `models`
    entries with USD per million tokens for input, cache read, cache write and
    output.
  - Validate: finite, non-negative, unique exact model IDs.
  - Provide `cost(model, &AgentUsage)`.
- [x] `crates/forgetest-agents/src/lib.rs`
  - Per-parser normalization (Claude `result`, Codex `turn.completed`; generic
    keeps the current keys).
  - Cost precedence: vendor first, then pricing.
  - Budget check: tokens = uncached + cache read + cache write + output.
  - With a cap and no usage: when `require_enforceable_usage` is set, return
    `BudgetUnverifiable` as `agent_error`.
- [x] `crates/forgetest-core/src/repository_report.rs`
  - Add `max_run_cost_usd` and an inline `pricing` to the policy parameters.
  - Add `AgentAggregate.cost_unavailable_trials`, which is `Some` only when a
    trial carries `cost_source`.
  - All of these are optional and skipped when `None`.
  - Integrity recomputes them.
- [x] `crates/forgetest-core/src/repository_engine.rs`
  - A dispatch loop that checks the breaker before each job.
  - Synthesize `cancelled` trials with zero attempts.
  - Keep usage that is known on failure paths.
- [x] `crates/forgetest-cli/src/{main.rs,commands/repository_run.rs,commands/agents.rs,commands/demo.rs}`
  - Add `--pricing <path>` and `--max-run-cost-usd`.
  - Add both to the policy.
  - Benchmark enforceability preflight in `run` and `agents lock`; the
    development profile warns.
- [x] `crates/forgetest-report/src/html.rs` -- render "(n unavailable)" next
  to cost.
- [x] Tests:
  - Claude/Codex-shaped events, precedence and unavailable usage.
  - Pricing validation; digest unchanged when unused; old v2 report loads.
  - Breaker and unverifiable-cost cancellation.
  - CLI preflight rejection.
- [x] `docs/src/advanced.md`, `README.md` -- document pricing, `cost_source`,
  the run cap and its overshoot bound.

**Acceptance Criteria:**
- Given a report or lock produced before this change, when it is loaded,
  compared, redacted or validated, then digests and aggregates verify
  unchanged.
- Given identical inputs without `--pricing` or `--max-run-cost-usd`, when the
  policy is built, then its digest equals the pre-change digest.
- Given a public (redacted) report of a priced run, when it is loaded, then
  `verify_digest` succeeds and the pricing table is visible.
- Given the full CI command set, when run locally, then every step passes.

## Design Notes

- **Field meanings for new records:**
  - `input_tokens` is uncached input.
  - `cached_tokens` is cache reads.
  - `output_tokens` includes reasoning; `reasoning_tokens` is informational and
    never priced separately.
  - These are assumptions from vendor docs. Live golden fixtures, captured
    before pilot-r2, must confirm them; see Ask First.
- **Pricing is stored inline in the policy, not as a path.** Redaction rewrites
  paths, which would break `verify_digest`.
- **Overshoot bound:** with parallelism `p`, in-flight trials can exceed the run
  cap by at most (p−1) × the per-trial cap.

## Verification

**Commands:**
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --all-targets --locked` -- expected: all pass.
- `bash scripts/verify-sample-reports.sh && bash scripts/test-verify-bmad.sh && bash scripts/verify-bmad.sh` -- expected: pass with committed samples untouched.
- `cargo run --locked --bin forgetest -- demo --mode repository --runner local --output <fresh> --format all` -- expected: exit 0.

## Suggested Review Order

**Usage normalization and cost precedence (entry point)**

- Claude cost now comes from the top-level `total_cost_usd` on the final `result` event
  [`lib.rs:1419`](../../crates/forgetest-agents/src/lib.rs#L1419)

- Codex: uncached input = input − cached; malformed cache counts yield no usage
  [`lib.rs:1448`](../../crates/forgetest-agents/src/lib.rs#L1448)

- Vendor cost wins, then the exact-model pricing entry, else `unavailable`, never 0
  [`lib.rs:1515`](../../crates/forgetest-agents/src/lib.rs#L1515)

- Benchmark fails closed when a cap exists but usage cannot verify it
  [`lib.rs:1027`](../../crates/forgetest-agents/src/lib.rs#L1027)

**Usage model and pricing table**

- New `CostSource`; `None` stays legacy so old reports verify unchanged
  [`agent.rs:166`](../../crates/forgetest-core/src/agent.rs#L166)

- Token-cap definition: uncached + cache read + cache write + output
  [`agent.rs:135`](../../crates/forgetest-core/src/agent.rs#L135)

- The breaker only counts vendor or table costs that are finite and non-negative
  [`agent.rs:150`](../../crates/forgetest-core/src/agent.rs#L150)

- Strict, canonically sorted table; future retrieval dates rejected at parse time
  [`pricing.rs:42`](../../crates/forgetest-core/src/pricing.rs#L42)

**Policy, digest and report integrity**

- Optional policy terms skipped when unused, so pre-change digests stay byte-identical
  [`repository_report.rs:336`](../../crates/forgetest-core/src/repository_report.rs#L336)

- One validator for run cap and pricing, shared by the loader and the engine
  [`repository_report.rs:277`](../../crates/forgetest-core/src/repository_report.rs#L277)

- Aggregate counts unavailable costs only when trials carry a source
  [`repository_report.rs:530`](../../crates/forgetest-core/src/repository_report.rs#L530)

**Run-level breaker and dispatch**

- Policy validated before any trial directory exists
  [`repository_engine.rs:122`](../../crates/forgetest-core/src/repository_engine.rs#L122)

- Round-robin dispatch keeps truncation balanced across agents and tasks
  [`repository_engine.rs:147`](../../crates/forgetest-core/src/repository_engine.rs#L147)

- Breaker trips on the cap or on any unverifiable finished trial
  [`repository_engine.rs:881`](../../crates/forgetest-core/src/repository_engine.rs#L881)

**CLI preflight and operator signals**

- Benchmark rejects unenforceable caps before Docker; development warns honestly
  [`repository_run.rs:138`](../../crates/forgetest-cli/src/commands/repository_run.rs#L138)

- Run cap must be positive and finite; checked before any spend
  [`repository_run.rs:119`](../../crates/forgetest-cli/src/commands/repository_run.rs#L119)

- Fail-closed usage is tied to the benchmark profile
  [`repository_run.rs:284`](../../crates/forgetest-cli/src/commands/repository_run.rs#L284)

- New `--max-run-cost-usd` flag and its overshoot help text
  [`main.rs:76`](../../crates/forgetest-cli/src/main.rs#L76)

**Rendering and docs**

- Cost cell says "unavailable" honestly; per-model prices appear under Provenance
  [`html.rs:352`](../../crates/forgetest-report/src/html.rs#L352)

- Operator docs: run cap semantics, truncation as incomplete evidence
  [`advanced.md:200`](../../docs/src/advanced.md#L200)

- Migration note: existing token caps can bind sooner under the same digest
  [`advanced.md:244`](../../docs/src/advanced.md#L244)

- Unreleased changelog entries
  [`CHANGELOG.md:5`](../../CHANGELOG.md#L5)

**Tests (peripherals)**

- Pins the pre-change policy digest when the new options are unused
  [`repository_report.rs:352`](../../crates/forgetest-core/tests/repository_report.rs#L352)

- Committed v2 reports reload byte for byte
  [`repository_report.rs:376`](../../crates/forgetest-core/tests/repository_report.rs#L376)

- Benchmark Docker path prices Codex through the agent container (fake docker)
  [`cli_tests.rs:1739`](../../crates/forgetest-cli/tests/cli_tests.rs#L1739)

- Breaker cancels unstarted trials once the cap is reached
  [`repository_engine.rs:561`](../../crates/forgetest-core/tests/repository_engine.rs#L561)

- Claude normalization and vendor-first precedence
  [`adapters.rs:746`](../../crates/forgetest-agents/tests/adapters.rs#L746)
