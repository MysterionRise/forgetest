# Don approval of the CI Python pin correction, 2026-10-15

This decision record is outside the public mdBook. It approves a correction to
the CI governance gate delivered under `spec-bmad-repository-setup`.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-15T10:21:09Z`.
- Channel: an interactive Claude Code session.

## Problem

The gate pinned `python-version: "3.10.0"`. On PR #52, `actions/setup-python`
failed because "The version '3.10.0' with architecture 'x64' was not found for
Ubuntu 24.04." The oldest exact 3.10 build for ubuntu-24.04 x64 is 3.10.4.

## Don statements

- Pin choice: "Update to proper major LTS version."
- Review record: "Hash refresh + decision record." Only the ci.yml SHA-256 in
  `_bmad-output/project-context.md` changes. The 2026-08-29 review metadata
  and the verifier constants that pin it stay unchanged.

## Interpretation applied

CPython has no formal LTS release. The pin is the latest patch of the newest
mature stable line available as an exact build for ubuntu-24.04 x64:
**3.14.8**. Python 3.15.0, released days earlier, was left out as too new.
The pin stays exact because the verifier rejects floating versions.

## Changes

- `.github/workflows/ci.yml`: `python-version: "3.14.8"`. New SHA-256:
  `6e5a17af4a3e854459e0dea0e1595daee735f4394419b01137f8ed7315daee7b`.
- `scripts/bmad_verify_common.py`: `CANONICAL_CI_GATE` matches the new
  gate.
- `scripts/test-verify-bmad.sh`: the floating-version bypass guard now
  rejects `"3.14"` against the canonical `"3.14.8"`.
- Fixture `scripts/fixtures/bmad/base/.github/workflows/ci.yml` (new SHA-256
  `5240e2608301eaa4b3187f66bc1efa2f6715e8bcfdda1721aa17e958fffa98e6`), together with its `project-context.fixture` identity and
  the `payloads/ci-baseline-drift.tsv` rehash token.
- `_bmad-output/project-context.md`: refreshed ci.yml identity.

The baseline CI inventory in `bmad.lock.json` is unchanged, because it hashes
the workflow with the gate block removed.

## Evidence

- `scripts/test-verify-bmad.sh` passed all 211 assertions under Python 3.14.8,
  with no `python3.10` on PATH, as on the CI runner.
- It also passed under Python 3.10.0.
- `scripts/verify-bmad.sh` passed under both interpreters.

## Follow-up

The verifier still documents a Python >=3.10 minimum. CI no longer exercises
3.10, and today's local 3.10 run is the latest proof of that minimum. Raising
the documented minimum, or adding a 3.10 CI step, would need its own plan,
because the onboarding text is pinned by the verifier.
