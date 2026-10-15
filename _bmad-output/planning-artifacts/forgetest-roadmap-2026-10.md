---
id: forgetest-roadmap-2026-10
artifact_type: plan
status: approved
classification: repository-visible
owner: repository-don-001
scope: repository/roadmap/2026-10
created: 2026-10-13
---

# forgetest roadmap, October 2026 to April 2027

## Decision state

This plan records the outcome of a three-role planning roundtable (Product
Owner, AI CTO, Chief Technologist) held on 2026-10-13, and the four direction
decisions the repository Don selected in that session. The Don approved it on
2026-10-15; the artifact index records the approval. It does not authorize
implementation of any initiative. Each initiative below needs its own flat,
indexed plan approved under
[`governance-lean-bmm-ruling`](governance-lean-bmm-ruling.md) before work starts.

## Starting point

- v0.1.0 ships a working harness. It has no real-agent evidence: the evidence
  ledger row "Real-agent performance claim" reads "None"
  (`docs/src/evidence.md`).
- The default branch fails `cargo audit` on RUSTSEC-2026-0258 (`h2` 0.4.13,
  pulled in by `reqwest` through `forgetest-providers`).
- The BMAD setup is uncommitted, and its final gate (WP-014) is blocked on that
  advisory.
- Running either pre-registered study as written would change its protected
  tree. The pilot writes to `studies/rust-agent-pilot/run` and renames
  `RESULTS.template.md`. `scripts/bmad_verify_studies.py` hashes every file on
  disk. The pilot's `development` profile also conflicts with `SECURITY.md`.
- Usage metering is unreliable:
  - `usage_from_events` in `crates/forgetest-agents/src/lib.rs` reads only
    vendor cost fields, so Codex trials record USD 0.00.
  - Caps are evaluated only after the agent exits.
  - A missing usage value becomes zero.
- With 12 tasks, success-rate comparisons are underpowered. At 3 trials per
  task the realistic minimum detectable paired difference is about 30 to 40
  percentage points. At 5 trials it is about 26. Cost differences of about 25
  to 30 percent are detectable at 5 trials.

## Goals and constraints

- Goals for the next six months:
  1. Credible published results.
  2. Adoption as a CI tool.
  3. A portfolio and CTO showcase.
- Commercial work is out of scope.
- Paid-agent budget is under USD 100 initially.
- One maintainer works near full-time, assisted by coding agents.

## Positioning

forgetest is the vendor-neutral, test-graded regression gate for coding-agent
configuration changes, covering model, CLI version, context files and skills.

- It reports "inconclusive" whenever the data cannot support a verdict.
- Its evidence is metered, sealed and signed, so anyone can re-verify it.

Personas, in order:

1. **Evidence Auditor**, through month 3. An evaluation researcher or a tech
   lead writing an agent-selection memo.
2. **Agent Config Owner**, from month 4. A DevEx lead or Rust maintainer who
   owns the model pin, AGENTS.md and skills.
3. **Technical Evaluator**, throughout. A CTO, a hiring panel or a conference
   reviewer.

The CI gate judges cost, latency and tokens. Success rate acts as a guardrail
and is shown with its minimum detectable effect.

## Direction decisions selected by the Don (2026-10-13)

1. **Lean BMM.** Tier 3 is satisfied by one flat, indexed, spec-shaped plan per
   initiative. See `governance-lean-bmm-ruling`. The first such plan is
   `spec-h2-rustsec-2026-0258`.
2. **Successor studies.** `studies/rust-agent-pilot` and `studies/rust-agent-v1`
   stay frozen and unexecuted. The new studies are `rust-agent-pilot-r2` and
   `rust-agent-v1-r2`. Their results live outside every frozen tree.
3. **Remove the legacy path before v0.2.** Delete `forgetest-providers` and the
   snippet engine before the first crates.io release, and record the breaking
   change in `CHANGELOG.md`. First extract the configuration types that
   `forgetest-cli` imports from `forgetest_providers::config`.
4. **Sealed, metered runs are the publication standard.** Build a
   `forgetest meter` sidecar in month 2. Until it exists, cost is labelled
   "vendor-reported".

Defaults adopted unless the Don objects:

- Commit-reveal for newly authored tasks: graders stay private until 30 days
  after publication.
- Web tools are disabled in benchmark profiles.
- A standing paid-run policy (see the ruling).

## Six-month plan

### Month 1 (to 2026-11-13): green and honest

- **Governance.**
  - Approve the lean-BMM ruling and the paid-run policy.
  - Bump the `h2` lockfile entry and replace the yanked `chacha20`.
  - Commit the BMAD setup and close WP-014.
  - Prove that the CI `python-version: "3.10.0"` pin resolves on hosted runners.
- **Measurement stage 0.**
  - Normalize usage into uncached input, cache read, cache write, output and
    reasoning tokens.
  - Price Codex `turn.completed` usage from a `pricing.toml` that is included in
    the policy digest.
  - Add `cost_source` (`vendor`, `pricing-table`, `unavailable`). Missing usage
    is never zero.
  - Add a run-level cost breaker, checked between trials.
  - Benchmark preflight rejects caps it cannot enforce.
- **Hardening.**
  - Disable web tools in the built-in profiles: Codex
    `-c web_search="disabled"`, Claude `--disallowedTools WebSearch,WebFetch`.
  - Capture golden stream fixtures from about USD 0.50 of live smoke calls.
  - Write agent Dockerfiles that carry Rust 1.92, with digests taken from a
    local registry.
  - Add a CI job that runs the benchmark profile with the scripted agent.
- **Study.**
  - Register `rust-agent-pilot-r2`: sealed benchmark profile, models confirmed
    available by a live call.
  - Run it for about USD 10 and audit the traces.
  - Publish a methods note, not a performance claim.
- **Exit:** the default branch is green, the methods note is published, and the
  ledger row no longer reads "None".

### Month 2: seal and slim

- **Build the `forgetest meter` sidecar.**
  - Same binary, distroless image, internal-only Docker network.
  - Vendor API host allowlist, with key injection so the agent container never
    holds an API key.
  - Reads SSE usage, kills the run at its budget, and adds `meter.jsonl` to the
    manifest.
  - HTTP/1.1 with rustls outbound.
  - Codex goes through a custom provider with WebSockets disabled.
- **v0.2 release.**
  - Remove the legacy providers and snippet engine.
  - Publish to crates.io with OIDC trusted publishing.
  - Ship `dist` installers, cargo-binstall support and Homebrew.
  - Fix or retire `release.sh` in favour of `scripts/publish-crates.sh`.
  - Pin the verifier base image by digest.
- **Paid-study workflow.** Shard the runs and sign trial records with
  `actions/attest` and a custom predicate.
- **Talk.** Submit a FOSDEM 2027 proposal. The deadline is expected around early
  December and has not been verified yet.
- **Exit:** meter and stream token counts agree within ±2 percent on 10 smoke
  trials, and cargo-binstall to a first report takes under 10 minutes.

### Month 3: evidence

- **Report schema v3**, additive so v2 reports still load. New fields:
  - a verdict from an exact sign-flip test, with the minimum detectable effect;
  - `cost_source`;
  - deterministic integrity flags:
    - edits to grader or test paths;
    - blocked egress;
    - declared server-side tools;
    - the answering model differs from the requested one;
    - git-history commands;
  - `overlay_digest`;
  - the study ID and protocol tree hash.
- **Study `rust-agent-v1-r2`.**
  - 12 tasks × 2 agents × 5 trials, using economy-tier models.
  - The meter is required.
  - The pre-registration is Rekor-timestamped.
  - Budget is about USD 30.
- **Essay.** Draft "Your agent eval can't tell 15 points apart".
- **Exit:** v1-r2 is published with attestations.

### Month 4: the CI gate

- **AgentSpec.** Split `crates/forgetest-agents/src/lib.rs` into modules and
  introduce `AgentSpec { label, adapter, model, effort, overlay }`.
- **Context-file overlays.** Apply them before the baseline snapshot and include
  them in the profile digest.
- **Lock schema v2**, keyed by label. Today duplicate agent names are rejected in
  `crates/forgetest-cli/src/commands/repository_run.rs`.
- **Verdict engine:** improved, regressed or inconclusive.
- **`forgetest plan`**, a power calculator.
- **GitHub Action.** A composite action that runs only on `workflow_dispatch` or
  a schedule and writes a job summary. It never uses `pull_request*` triggers.
- **Exit:** three external repositories have tried the gate.

### Month 5: corpus

- **Verifier contract.** A language-agnostic contract covering image digest,
  command, and reward or exit code, with no network.
- **Import.** The 25-task Rust subset of SWE-rebench (CC BY 4.0), via Harbor.
- **Bring-your-own-repo MVP.**
  - `task new --from-commit`.
  - A vendored-dependency store keyed by the `Cargo.lock` hash.
  - Verifier limits raised and included in the policy digest. Today they are
    1 GiB RAM and a 256 MiB build directory in
    `crates/forgetest-runner/src/repository_grader.rs`.
  - Reference patches re-run five times to catch flaky tests.
- **Canaries.** Three canary tasks.
- **E7 pre-registration:** a Rust replication of the published finding that
  AGENTS.md-style context files raise cost.
- **Exit:** at least 35 calibrated tasks.

### Month 6: showcase

- **E7.** Run and publish it, if budget is available.
- **`forgetest analyze <harbor-job>`.** Wilson intervals, pass^k, the sign-flip
  test and minimum detectable effect for Harbor runs in any language.
- **More agents.** An OpenCode adapter, then Gemini CLI, written as declarative
  stream specs.
- **Case study.** Refresh it with real numbers.
- **Exit:** two dated studies published.

## Budget

| Item | Estimate (USD) |
|---|---:|
| Live smoke calls | 5 |
| `rust-agent-pilot-r2` | 10 |
| `rust-agent-v1-r2` | 30 |
| E7 (needs a top-up) | 30 |
| Reserve | 25 |

Provider-console spend limits are the hard stop. In-harness caps are a second
line.

## Not doing in the next six months

- Leaderboards or frontier head-to-head runs at USD 10 per trial.
- LLM-judge grading.
- An OpenTelemetry receiver, LiteLLM, or any third-party gateway.
- ATIF as the internal trace format.
- Listing the current 12-task corpus in the Harbor registry.
- A hand-authored multi-language corpus.
- A standalone security track or any claim of hostile-repository safety.
- Subscription-authenticated published runs.
- WASM or dylib plugins.
- Commercial work.
- The schema-v2 plan `bmad-artifact-workspace-compatibility`. It stays parked
  until the batched governance amendment in month 3.

## Success measures (month 6)

| Goal | Measures |
|---|---|
| Published results | Two dated, attested studies; at least one independent citation; no retractions. |
| CI adoption | At least 10 external repositories using the Action; at least 300 crates.io downloads per month; at least five external issues or pull requests. |
| Showcase | CI green on at least 95 percent of days; a talk accepted or a post with at least 5,000 views. |
| Governance | Overhead at or below 10 to 15 percent of capacity, tracked weekly. |

## Risks

| Risk | Mitigation |
|---|---|
| Rulings stall and CI stays red | Decide in week 1 |
| Python 3.10.0 is unavailable on hosted runners | Prove it on a branch |
| Misleading cost numbers | `cost_source` and the meter |
| The hidden grader leaks through the host or the web | Containers, web tools off, trace audit, commit-reveal |
| Model access or retirement | Live smoke call; choose models with the longest retirement horizon |
| Underpowered results | 5 trials per task, cost and latency gating, published minimum detectable effect |
| CLI stream drift | Golden fixtures and a parser version in the agent identity |
| Solo scope creep | The not-doing list above |

## Research basis

The roundtable used public sources current to October 2026:

- SWE-bench Verified was retired (2026-02-23).
- SWE-bench Pro audits found roughly 30 percent of tasks broken (2026-07-08).
- SWE-Bench Pro Verified (arXiv 2609.08149).
- Cursor's reward-hacking analysis: about 57 percent of audited runs fetched the
  upstream fix.
- Harbor added separate verifier sandboxes (2026-05-15) and maintains a dataset
  registry that includes SWE-rebench 07-2026.
- Rust-SWE-bench (arXiv 2602.22764).
- An ETH Zurich context-file study (February 2026).
- SkillsBench (arXiv 2602.12670).
- The JetBrains 2026 agent-adoption survey.
- Cloud Security Alliance research notes on prompt injection through CI agents.
- The LiteLLM PyPI compromise (2026-03-24).
