# Case Study: From Snippet Demo to Agent Evidence Platform

## Executive Summary

The original `forgetest` answered a useful but limited question: can an LLM
generate a Rust function that compiles and passes embedded tests? That proved
provider integration and Rust execution, but it did not resemble how coding
agents modify repositories, and weighted snippet scores were too weak to
support leadership-level claims.

The v1 redesign changes the unit of evaluation from a completion to a verified
repository change. An agent receives a visible workspace and prompt. The
trusted harness records its trace, captures its filesystem changes independent
of Git, applies them to a clean workspace, overlays a protected grader, and
records a categorical outcome plus inspectable evidence.

## Product Decision

The project is not positioned as a universal leaderboard. It is a regression
and evidence harness for a team choosing or upgrading coding-agent
configurations on a controlled Rust workload.

That scope supports concrete decisions:

- Did an agent upgrade regress our fixed task suite?
- Is a result repeatable across three independent trials?
- Was a failure caused by the patch, the agent process, or infrastructure?
- Can a reviewer trace the exact model, CLI, image, policy, patch, and grader
  evidence behind a number?

## Architectural Decisions

### Binary repository outcomes

All required fail-to-pass and pass-to-pass checks must succeed. Weighted
compile/test/structure/Clippy scores remain only for legacy snippets and
diagnostics.

### Dual execution environments

Local mode optimizes trusted iteration. Published mode uses an ephemeral agent
container and an independent, network-disabled verifier. The agent may access a
hosted API but cannot see the hidden grader or grade its own workspace.

### Content and policy identity

Suite, task, agent profile, and full security/budget policy receive SHA-256
identities. A benchmark lock freezes immutable images, exact CLI binaries and
versions, requested model IDs, effort settings, and the policy. Profile or
observed binary/image drift is an error; provider-side routing remains
observable only when vendor output reports it.

### Evidence before dashboard

Every scheduled trial is atomically persisted. Private evidence retains typed
and unknown vendor events, patches, grader logs, usage, and termination.
Redacted evidence removes private model output and credential/path material.
Each bundle has a deterministic checksum inventory.

### Honest statistics

Reports separate task, agent, and infrastructure failures. They show Wilson
intervals, first-trial success, all-three reliability, and task-paired
bootstrap comparisons. Small-corpus uncertainty remains visible.

## Security Judgment

The design does not claim that containers make hostile native code safe.
Instead it defines a defensible boundary for audited fixtures:

- Explicit configuration trust boundary.
- Fresh workspaces and per-trial caches.
- Hidden grader unavailable to the agent.
- Environment allowlists and isolated homes.
- Network-disabled verifier.
- Non-root, read-only, capability-free containers with resource limits.
- Unix process-group and forced container cleanup on abnormal termination.

The residual kernel/runtime/compiler and agent-network risks are documented in
`SECURITY.md`.

## Corpus

The 12-task Rust corpus contains eight authored fixtures and four reduced,
licensed upstream adaptations. It covers bug fixes, features, API migrations,
async/concurrency, and security/robustness.

Corpus admission requires:

1. Strict schema and provenance.
2. Null patch fails the intended fail-to-pass check.
3. Reference patch passes all fail-to-pass and pass-to-pass checks.
4. Hidden grader remains outside the visible workspace.

CI runs this calibration as trusted local code.

## What CI Proves

For a green commit, the checked-in workflow proves:

- Formatting, all-target lint/test, docs, and package gates.
- RustSec, license, and source checks.
- All 12 corpus controls.
- Deterministic local snippet and repository demos.
- Docker image build and gated execution.
- Docker snippet and independent repository verification.
- Installed-binary smoke behavior.
- Upload of private/public evidence artifacts.

It does not prove a paid Codex or Claude benchmark. Those require external
credentials, immutable agent images, selected release-candidate models, cost
authorization, and a dated run.

## Study Protocol

The v1 acceptance study is:

```text
12 tasks x 2 locked agent configurations x 3 trials = 72 trials
```

The publication must include:

- Date, commit, suite digest, benchmark lock, and workflow/run environment.
- Exact agent CLI/model/effort/image identities.
- Per-task/per-trial status matrix.
- Success rate and Wilson 95% interval.
- Pass@1 and pass^3.
- Task-paired difference and bootstrap 95% interval.
- Reported token/cost totals and infrastructure failures.
- Redacted traces, patches, grader evidence, checksums, and limitations.

No values are pre-filled. The repository currently contains protocol and
tooling, not fabricated model results.

## Remaining Work Before v1 Evidence

- Build and review vendor-specific agent images.
- Select exact release-candidate models and effort settings.
- Conduct an external security/architecture review.
- Execute the locked 72-trial run on a disposable worker.
- Review redaction manually.
- Publish checksums, SBOM, release provenance, protocol, and public evidence.

This is the deliberate line between a credible engineering artifact and an
unsupported benchmark claim.
