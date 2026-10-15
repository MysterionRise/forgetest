---
title: 'Remediate RUSTSEC-2026-0258 and October 2026 RustSec advisories with lockfile-only updates'
type: 'chore'
created: '2026-10-13'
status: 'done'
baseline_commit: '88385f60adb116a04fc3834152c6d40f7b8fb916'
context:
  - '{project-root}/_bmad-output/project-context.md'
  - '{project-root}/_bmad-output/planning-artifacts/governance-lean-bmm-ruling.md'
  - '{project-root}/_bmad-output/implementation-artifacts/deferred-work.md'
---

## Intent

**Problem:** `cargo audit` fails on the default branch because RUSTSEC-2026-0258
affects `h2` 0.4.13. The fix requires `h2` 0.4.16 or newer, and distribution
notices cite 0.4.17. `h2` is a transitive dependency: it reaches
`forgetest-cli` through `hyper`, `reqwest` and `forgetest-providers`. As a
result:

- the CI quality job fails;
- the final gate (WP-014) of `spec-bmad-repository-setup` is blocked;
- the matching `deferred-work` entry stays open.

`Cargo.lock` also pins `chacha20` 0.10.1, which `cargo audit` reports as yanked.

**Amendment (2026-10-15):** after the `h2` update, `cargo audit` still
reported nine advisories in packages the lockfile already pinned:

- `quinn-proto` 0.11.16, five advisories: RUSTSEC-2026-0338, 0339, 0340,
  0341 and 0342. Two are rated 7.5 (high). Fixed in 0.11.18.
- `rustls` 0.23.36, four advisories: RUSTSEC-2026-0285, 0346, 0348 and
  0349. Fixed in 0.23.46.

These packages reach the build only through `reqwest` and
`forgetest-providers`. The Don amended this plan to cover them under the same
lockfile-only rules, together with the `rustls-webpki` patch release that
`rustls` 0.23.46 resolves to.

**Approach:**

1. Run `cargo update -p h2 --precise <version>`, choosing the newest compatible
   0.4.x release at or above 0.4.16.
2. Replace the yanked `chacha20` entry with a non-yanked compatible release, if
   one exists.
3. Amendment: update `quinn-proto` to 0.11.19, `rustls` to 0.23.46 and
   `rustls-webpki` to 0.103.15.
4. Run the full regression.

Both changes are lockfile-only. Removing `forgetest-providers` before v0.2,
recorded in `forgetest-roadmap-2026-10`, will later remove `h2` from the graph
entirely. This plan is the interim fix.

## Boundaries & Constraints

**Always:**
- Change only `Cargo.lock`.
- Keep Rust 1.92 and every manifest unchanged.
- Keep the authoritative-source identities in
  `_bmad-output/project-context.md` byte-identical.
- Run the full CI proof list locally before any push.

**Ask First:**
- Any update that needs a `Cargo.toml` change.
- Any update that pulls in a semver-incompatible release or another advisory.
- Any update that pulls in a new dependency source or license that
  `cargo deny` rejects.
- Any change to `.cargo` audit configuration or ignore lists.

**Never:**
- Silence the advisory with an ignore entry.
- Modify protected studies.
- Weaken CI.
- Edit authoritative sources under this plan.

## I/O & Edge-Case Matrix

| Case | Expected outcome |
|---|---|
| `h2` 0.4.16+ resolves with existing manifests | Lockfile updated; `cargo audit` reports no vulnerability |
| The update cascades to other crates | Accept only semver-compatible lockfile movement; review the diff and record it in the commit message |
| No non-yanked `chacha20` is available | Keep 0.10.1, record it as an allowed warning, and track it in `deferred-work` |
| A new advisory appears after the update | Stop and ask; do not add ignores |
| An upstream yank lands after the update | Re-run `cargo audit` before merge |
| Re-resolving also re-points unrelated `windows-sys` edges | Keep the baseline edges. Change only the updated packages' version and checksum lines, and prove consistency with `cargo metadata --locked` |

## Tasks

1. Update `h2`, `chacha20`, `quinn-proto`, `rustls` and `rustls-webpki` in
   `Cargo.lock`.
2. Run the regression:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --all-targets --locked`
   - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`
   - `cargo audit`
   - `cargo deny check licenses sources`
   - `forgetest validate --suite eval-suites/rust-agent-v1/suite.toml --calibrate`
   - `bash scripts/test-verify-bmad.sh`
   - `bash scripts/verify-bmad.sh`
3. Re-run WP-014 of `spec-bmad-repository-setup` and resolve the `deferred-work`
   entry.

## Acceptance Criteria

- `cargo audit` reports no vulnerabilities. Only allowed warnings remain.
- Every command listed in the tasks passes locally.
- All five CI jobs pass on the pull request.
- Protected-study tree identities are unchanged.
- `git diff` against the baseline touches `Cargo.lock` and no product source.
