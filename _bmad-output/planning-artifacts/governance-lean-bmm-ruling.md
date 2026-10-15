---
id: governance-lean-bmm-ruling
artifact_type: plan
status: approved
classification: repository-visible
owner: repository-don-001
scope: repository/bmad-governance/lean-bmm-ruling
created: 2026-10-13
---

# Lean BMM ruling for the single-maintainer period

## Decision state

On 2026-10-13 the repository Don selected the lean-BMM approach to unblock
dependency remediation, and approved this ruling as written on 2026-10-15. The
artifact index records the approval, approval evidence and the
independence-exception fields for this ID. It changes no verifier, policy
mirror, authority registry or authoritative source.

## Problem

Tier 3 requires full BMM planning for dependency changes. In this repository
that requirement cannot be met:

- The installed BMM workflows write multi-file outputs such as `prd.md`,
  `epics.md` and `sprint-status.yaml`.
- The artifact verifier rejects every unindexed Markdown file under a
  canonical root.
- It also rejects any file whose name stem differs from its index ID.
- The planned escape, `bmad-artifact-workspace-compatibility`, needs two Don
  approvals and a verifier rewrite.

As a result, the RUSTSEC-2026-0258 remediation, and with it the final gate of
`spec-bmad-repository-setup`, has no lawful planning path.

A second issue: the authority registry holds one human identity, and no one
holds the `repository-maintainer` role. Conflict escalation to a
repository maintainer therefore has no recipient.

## Ruling

R1. **Meaning of full BMM planning.**
- While the registry holds a single active human identity, an initiative in a
  tier 3 category is fully planned when it has exactly one flat, spec-shaped
  plan artifact. That artifact:
  - is registered in `_bmad-output/index.yaml` under its own ID at a canonical
    root;
  - states its intent, its boundaries (always, ask first, never), its I/O and
    edge-case matrix, its tasks, its acceptance criteria and its verification;
  - is approved by the Don with the independence exception before
    implementation starts.
- Producing the plan with `bmad-spec`, or with an equivalent installed BMM
  workflow, is sufficient.
- Multi-file BMM outputs are optional. They may be written to a canonical root
  only if every file is individually indexed. Transient workflow output stays
  outside `_bmad-output`.

R2. **Conflict review.** While no active identity holds `repository-maintainer`,
the Don performs the repository-maintainer review that conflicting
authoritative sources require. The stop-and-review obligation is unchanged.

R3. **Paid-run policy.** Paid agent runs are allowed only under all of these
conditions:
- They run under a Don-approved study or initiative plan.
- Scoped API keys are created for that plan, with spend limits set in each
  provider console, and are revoked afterwards.
- Total paid spend stays within USD 100 until the Don records a top-up in the
  roadmap.
- The plan names a per-run cap.
- The run uses a GitHub-hosted ephemeral runner or a disposable machine.
- Published runs never use subscription or interactive login credentials.
- Benchmark profiles disable web tools.

Each run produces a run record with:
- the plan ID;
- the commit;
- the lock digest;
- the cap;
- the reported spend;
- the provider-console spend.

A run that stays within these conditions needs no further approval.

R4. **Security remediation.** A RustSec advisory reported by `cargo audit` on
the default branch is remediated within five Monday-to-Friday days
(Europe/London), through a flat plan under R1. An advisory exploitable in
forgetest's own threat model uses the tier 2 urgent-security procedure instead.

R5. **Overhead cap.** Governance work should stay at or below 10 to 15 percent
of maintainer capacity, reviewed weekly. Sustained overrun triggers the batched
amendment in R6 early.

R6. **Batched amendment.** `bmad-artifact-workspace-compatibility` stays a
parked draft. One batched governance amendment is planned for month 3 of
`forgetest-roadmap-2026-10`. It will cover:
- narrowing tier 3;
- any verifier constant updates;
- registering a second human authority, if one becomes available.

R7. **Lapse.** This ruling lapses when either of these happens:
- an active `repository-maintainer` identity is registered;
- the R6 amendment is approved.

## Boundaries

- **Always:**
  - Protected studies stay immutable; tier 1 is unchanged.
  - Repository authorities override this ruling.
  - Every approval is recorded in the index with evidence.
- **Never:**
  - Treat chat-session choices as recorded approval.
  - Weaken CI.
  - Change `AGENTS.md`, `CLAUDE.md`, the policy mirror or the verifier under
    this ruling.

## Verification

`bash scripts/verify-bmad.sh` passes with this artifact and its approval record
indexed.
