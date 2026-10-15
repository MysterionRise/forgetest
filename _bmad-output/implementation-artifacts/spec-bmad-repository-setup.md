---
title: 'Finish repository-wide BMAD setup'
type: 'chore'
created: '2026-08-15'
status: 'in-review'
baseline_commit: '88385f60adb116a04fc3834152c6d40f7b8fb916'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/project-context.md'
  - '{project-root}/docs/gangsta/bmad-repository-setup/specs/2026-08-14-contract.md'
  - '{project-root}/docs/gangsta/bmad-repository-setup/plans/2026-08-14-execution-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The approved BMAD 6.11.0 foundation, shared launchers, governance,
context, and onboarding exist, but the verifier and CI gate are missing, so the
32-case governance harness remains intentionally red.

**Approach:** Preserve the accepted exact installation and human-owned policy,
implement one deterministic verifier against production-shaped fixtures, add it
to CI without changing the baseline proof contract, then prove disposable
reproducibility and regression safety.

## Boundaries & Constraints

**Always:** Treat repository authorities in `project-context.md` as superior to
generated guidance; preserve the exact lock and BMM-only scope; keep generated
and human-owned paths disjoint; develop each verifier slice RED→GREEN; use one
Bash 3.2 entrypoint plus Python 3.10+ standard-library helpers; emit stable
diagnostics; support shallow checkouts; and register durable work in the index.

**Ask First:** Any change to the signed intent, installer version or modules,
authority/classification model, CI baseline semantics, publication boundary, or
Git commit/push/PR. Minor or major BMAD updates require a new approved plan.

**Never:** Run installation or regeneration in this working checkout; modify
product code, protected studies, benchmark meaning, or public mdBook pages;
weaken existing CI; track local tool state; accept unsafe paths; add network or
non-stdlib verifier dependencies; commit prohibited data; or copy concrete
internal contract identifiers outside `docs/gangsta/`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Valid checkout | Lock, governance, and authorities agree | Matrix and verifier pass | No mutation |
| Lock/path drift | Mismatch, extra path, traversal, symlink, duplicate, overlap | Reject before later reads | Name invariant and relative path |
| Invalid governance | Bad authority, approval, lifecycle, scope, class, or content | Reject record | Stable, non-sensitive diagnostic |
| Unsafe surface | Launcher drift, local state, study instruction, or publication | Reject surface | Name category and path |
| Context/CI drift | Authority digest or baseline CI changes | Require review or restoration | Marked additive gate is allowed |
| Shallow checkout | History absent; committed evidence intact | Pass from normalized evidence | Never require remote history |

</frozen-after-approval>

## Code Map

- `bmad.lock.json`, `_bmad/_config/manifest.yaml`, and both launcher roots --
  accepted immutable identities and generated inventory; validate, do not
  regenerate in place.
- `_bmad-output/{authorities.yaml,index.yaml,project-context.md}` -- canonical
  authority, artifact lifecycle, and source identities.
- `AGENTS.md`, `CLAUDE.md`, `README.md` -- accepted policy parity and onboarding.
- `scripts/test-verify-bmad.sh`, `scripts/fixtures/bmad/` -- RED harness and
  negative cases; migrate to one production schema and preserve diagnostics.
- `scripts/verify-bmad.sh` and focused helpers -- single public verifier.
- `.github/workflows/ci.yml` -- five-job/34-step baseline; add only a uniquely
  marked, removable gate.
- `docs/gangsta/bmad-repository-setup/` -- signed intent, plan, and audit history.
- `studies/**`, `docs/src/**`, Rust sources -- read-only regression boundaries.

## Tasks & Acceptance

**Execution:**
- [x] Lock, inspect, reproduce, sanitize, and promote BMAD 6.11.0; add matching
  launchers, human governance, context, policy, and onboarding.
- [x] `scripts/verify-bmad.sh`, focused helpers, harness, and fixtures -- enforce
  lock/path, governance, launcher/study/publication, and drift contracts.
- [x] `.github/workflows/ci.yml` and `_bmad-output/project-context.md` -- add the
  marked gate and refresh only the reviewed current CI identity.
- [x] Package-audit evidence and regression checks -- reproduce in a disposable
  clone, run all applicable proof gates, and confirm protected paths unchanged.

**Acceptance Criteria:**
- Given the installation and governance, when both BMAD commands run, then the
  checkout passes and every negative fixture fails for its named invariant.
- Given the CI change, when the marked BMAD block is removed byte-for-byte, then
  the locked five-job/34-step baseline is reconstructed exactly.
- Given the lock in a disposable clone, when generation repeats, then generated
  identities match and no human-owned path changes.
- Given final proof gates, when completion is claimed, then product behavior,
  protected studies, and the public mdBook set are unchanged.

## Spec Change Log

- 2026-10-15: WP-014 gates pass.
  - `spec-h2-rustsec-2026-0258` (Don-approved, amended) remediated
    RUSTSEC-2026-0258 and the later quinn-proto and rustls advisories with
    lockfile-only updates.
  - The CI gate's exact Python pin moved from 3.10.0, which has no
    ubuntu-24.04 build, to 3.14.8. See
    `docs/governance/decisions/2026-10-15-don-approval-ci-python-pin.md`.
  - All seven checks of CI run 38719674660 passed on PR #52, including the
    BMAD governance gate, `cargo audit`, Docker proof and the Windows,
    macOS and ARM64 jobs. Protected studies and product sources are
    unchanged.
  - Operational status stays `in-review`. The harness assertion
    `durable-evidence-remains-blocked` in
    `scripts/fixtures/bmad/runtime-tests.sh` and the package-audit
    evidence in `docs/gangsta/` still record the blocked state. Moving
    to done requires a Don-reviewed update of both.

## Design Notes

Use duplicate-key-safe JSON for the JSON-syntax registries, narrow parsers for
manifest/front matter, and standard-library CSV. Validate paths and lock before
reading governance or launchers. Keep the historical CI baseline in the lock;
stripping the additive marker block must recover its exact bytes.

## Verification

**Commands:**
- `bash scripts/test-verify-bmad.sh && bash scripts/verify-bmad.sh` -- BMAD
  positive and negative contracts pass.
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --all-targets --locked` -- core Rust gates pass.
- Run remaining rustdoc, packaging, audit, portability, mdBook, local-proof, and
  applicable Docker-proof commands from the CI contract.

## Suggested Review Order

**Execution spine**

- Start with the deterministic, Bash-compatible public verifier entrypoint.
  [`verify-bmad.sh:1`](../../scripts/verify-bmad.sh#L1)

- Follow the coordinator that orders every validation boundary.
  [`verify_bmad.py:26`](../../scripts/verify_bmad.py#L26)

- Review immutable installation inventory and human-owned separation.
  [`bmad_verify_lock.py:32`](../../scripts/bmad_verify_lock.py#L32)

- Check exact package, dependency, generation, and promotion provenance.
  [`bmad_verify_provenance.py:46`](../../scripts/bmad_verify_provenance.py#L46)

**Governance invariants**

- Validate active human identities, role scopes, and signed evidence.
  [`bmad_verify_authority.py:104`](../../scripts/bmad_verify_authority.py#L104)

- Enforce artifact schema, lifecycle, indexing, and urgent-security routing.
  [`bmad_verify_governance.py:44`](../../scripts/bmad_verify_governance.py#L44)

- Treat the mirrored machine policy as one exact contract.
  [`bmad_verify_policy.py:293`](../../scripts/bmad_verify_policy.py#L293)

- Bind current and future protected studies to deterministic trees.
  [`bmad_verify_studies.py:78`](../../scripts/bmad_verify_studies.py#L78)

- Scan all governed surfaces without echoing forbidden values.
  [`bmad_verify_sensitive.py:28`](../../scripts/bmad_verify_sensitive.py#L28)

**Repository adoption**

- Read the repo-wide BMAD default and precedence rules first.
  [`AGENTS.md:3`](../../AGENTS.md#L3)

- Review observed project facts and agent operating constraints.
  [`project-context.md:89`](../project-context.md#L89)

- Inspect canonical authority and artifact lifecycle records together.
  [`authorities.yaml:1`](../authorities.yaml#L1)
  [`index.yaml:1`](../index.yaml#L1)

- Confirm the exact BMAD 6.11.0 BMM-only installation identity.
  [`bmad.lock.json:1`](../../bmad.lock.json#L1)

**CI, onboarding, and proof**

- Verify Pages rendering precedes the removable governance gate.
  [`ci.yml:47`](../../.github/workflows/ci.yml#L47)

- Follow contributor routing, runtime requirements, and completion commands.
  [`README.md:48`](../../README.md#L48)

- Exercise the tracked launcher without installer or regeneration.
  [`test-bmad-render.sh:1`](../../scripts/test-bmad-render.sh#L1)

- Audit the complete negative and adversarial regression harness.
  [`test-verify-bmad.sh:1`](../../scripts/test-verify-bmad.sh#L1)

- Finish with provenance evidence and the separately planned advisory.
  [`2026-08-14-bmad-package-audit.md:323`](../../docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md#L323)
  [`deferred-work.md:1`](deferred-work.md#L1)
