---
title: 'Remediate RUSTSEC-2026-0382 in rustls-webpki with a lockfile-only update'
type: 'chore'
created: '2026-10-26'
status: 'ready-for-dev'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-h2-rustsec-2026-0258.md'
  - '{project-root}/_bmad-output/project-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `cargo audit` fails on the default branch and on every open pull
request. RUSTSEC-2026-0382 (published 2026-10-20, "Certificate verification may
accept distrusted intermediates") affects `rustls-webpki` 0.103.15, which
`Cargo.lock` pins. The fix is in 0.103.16. The crate reaches the build only
through `rustls`, `hyper-rustls`, `reqwest` and the legacy
`forgetest-providers` crate, which S6 removes before v0.2. The CI quality job
fails until the lockfile moves.

**Approach:** run `cargo update -p rustls-webpki --precise 0.103.16`. A dry
run on 2026-10-26 showed that this moves only `rustls-webpki`. Then run the
full regression. This is the interim fix; removing `forgetest-providers` in S6
removes the dependency path.

## Boundaries & Constraints

**Always:**
- Change only `Cargo.lock`, apart from this spec, its index record and its
  decision record.
- Keep Rust 1.92 and every manifest unchanged.
- Keep the authoritative-source identities in
  `_bmad-output/project-context.md` byte-identical.
- Run the full CI proof list locally before any push.

**Ask First:**
- Any update that needs a `Cargo.toml` change.
- Any update that moves another package, pulls in a semver-incompatible
  release, or surfaces another advisory.
- Any change to audit or deny configuration, or to ignore lists.

**Never:**
- Silence the advisory with an ignore entry.
- Modify protected studies.
- Weaken CI.

## I/O & Edge-Case Matrix

| Case | Expected outcome |
|---|---|
| `rustls-webpki` 0.103.16 resolves with existing manifests | Only its version and checksum lines change in `Cargo.lock`; `cargo audit` reports no vulnerability |
| The update moves any other package | Stop and ask |
| A new advisory appears | Stop and ask; add no ignores |
| The existing `tokio` unsoundness warning (RUSTSEC-2026-0386) | Unchanged: it is an allowed warning, not a vulnerability, and out of scope |

</frozen-after-approval>

## Tasks & Acceptance

**Execution:**
- [ ] `Cargo.lock`: update `rustls-webpki` to 0.103.16, and confirm with
  `cargo metadata --locked` that nothing else moved.
- [ ] Run the full local CI proof list: fmt, clippy, rustdoc, Pages,
  `test-verify-bmad.sh`, `verify-bmad.sh`, the publish test, package dry run,
  `cargo audit`, `cargo deny`, `cargo test`, calibration, sample reports and
  both demos.

**Acceptance Criteria:**
- `cargo audit` reports no vulnerabilities; only allowed warnings remain.
- Every command in the proof list passes locally.
- All CI jobs pass on the pull request.
- `git diff` against the baseline touches `Cargo.lock` and governance files
  only; protected-study tree identities are unchanged.

## Verification

**Commands:**
- `cargo update -p rustls-webpki --precise 0.103.16` -- expected: only
  `rustls-webpki` updated.
- `cargo audit` -- expected: no vulnerabilities.
- `bash scripts/verify-bmad.sh` -- expected: pass.
