---
title: 'November 2026 dependency refresh: RUSTSEC-2026-0382 and four Dependabot bumps, lockfile-only'
type: 'chore'
created: '2026-10-26'
status: 'done'
baseline_commit: 'ce731fa8b8b856550daed905ee55bd084852826f'
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
`forgetest-providers` crate, which S6 removes before v0.2.

Four Dependabot pull requests have been open since 2026-10-13:

- #48 `tempfile`;
- #49 `serde_json`;
- #50 `thiserror`;
- #51 `anyhow`.

Each is lockfile-only. They fail CI on advisories that master has since fixed,
and they would still fail on RUSTSEC-2026-0382. On 2026-11-04 the Don chose to
batch them into this spec.

**Approach:** all lockfile-only, with no manifest change.

1. `cargo update -p rustls-webpki --precise 0.103.16`. Already done and
   reviewed on 2026-10-26; only `rustls-webpki` moved.
2. `cargo update -p anyhow -p thiserror -p serde_json`.
3. `cargo update -p tempfile --precise 3.27.0`. A plain `-p tempfile` update
   does not move it.

Dry runs on 2026-11-04 showed this movement:

| Package | From | To | Note |
|---|---|---|---|
| `anyhow` | 1.0.103 | 1.0.104 | |
| `serde_json` | 1.0.149 | 1.0.154 | |
| `thiserror` and `thiserror-impl` | 2.0.18 | 2.0.21 | adds `syn` 3.0.9 alongside `syn` 2.x |
| `tempfile` | 3.25.0 | 3.27.0 | |
| `rustix` | 1.1.3 | 1.1.6 | moved by `tempfile` |
| `linux-raw-sys` | 0.11.0 | 0.12.1 | moved by `tempfile` |

After the refresh, close #48–#51 as superseded.

## Boundaries & Constraints

**Always:**
- Change only `Cargo.lock`, apart from this spec, its index record and its
  decision records.
- Keep Rust 1.92 and every manifest unchanged.
- Keep the authoritative-source identities in
  `_bmad-output/project-context.md` byte-identical.
- Run the full CI proof list locally before any push.

**Ask First:**
- Any update that needs a `Cargo.toml` change.
- Any package movement beyond the set listed in the Approach.
- Any semver-incompatible release, apart from the `syn` 3.0.9 that
  `thiserror-impl` 2.0.21 requires.
- Any new advisory, or any new license or source that `cargo deny` rejects.
- Any change to audit or deny configuration, or to ignore lists.

**Never:**
- Silence an advisory with an ignore entry.
- Modify protected studies.
- Weaken CI.

## I/O & Edge-Case Matrix

| Case | Expected outcome |
|---|---|
| The listed set resolves with existing manifests | Only those packages' entries change in `Cargo.lock`; `cargo audit` reports no vulnerability |
| `thiserror-impl` 2.0.21 needs `syn` 3.0.9 | Accepted: both `syn` majors are locked; build with `--locked` on Rust 1.92; `cargo deny` passes |
| Re-resolution re-points `windows-sys` or similar edges to versions already in the lockfile | Keep the baseline edge unless an updated package's own manifest requires the new one; prove consistency with `cargo metadata --locked` |
| Any other package moves | Stop and ask |
| A new advisory appears | Stop and ask; add no ignores |
| Existing allowed warnings: `tokio` RUSTSEC-2026-0386 (unsound) and `pin-utils` unmaintained, if listed | Unchanged and out of scope; record them |

</frozen-after-approval>

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.lock`: `rustls-webpki` 0.103.16 (done 2026-10-26; reviewed).
- [x] `Cargo.lock`: the four Dependabot bumps as listed in the Approach, then
  confirm with `cargo metadata --locked` and a before/after package list that
  nothing outside the listed set moved.
- [x] Run the full local CI proof list: fmt, clippy, rustdoc, Pages,
  `test-verify-bmad.sh`, `verify-bmad.sh`, the publish test, package dry run,
  `cargo audit`, `cargo deny`, `cargo test`, calibration, sample reports and
  both demos.
- [x] Add a verification record below: commands, exit status, toolchain,
  `cargo audit` before and after, and the package movement list.
- [ ] After the Don merges this change: close Dependabot #48–#51 as
  superseded, once the Don says to. Each is matched or exceeded here:
  - #48 `tempfile` 3.27.0: same;
  - #49 `serde_json` 1.0.151: 1.0.154 here, which Dependabot's own lockfile
    also used;
  - #50 `thiserror` 2.0.21: same;
  - #51 `anyhow` 1.0.104: same.

**Acceptance Criteria:**
- `cargo audit` reports no vulnerabilities; only allowed warnings remain.
- Every command in the proof list passes locally.
- All CI jobs pass on the pull request.
- `git diff` against the baseline touches `Cargo.lock` and governance files
  only; protected-study tree identities are unchanged.

## Spec Change Log

- **2026-11-04, Don decision: batch the Dependabot bumps.** The frozen
  content was amended, and the Don re-approved it at CHECKPOINT 1.
  - Trigger: four lockfile-only Dependabot pull requests had been stale for
    three weeks.
  - Frozen changes:
    - The title now names the whole refresh. The artifact ID, file name and
      index scope keep their original names for continuity.
    - The problem statement adds the Dependabot pull requests and drops the
      sentence about the CI quality job; CI is still red until this merges.
    - The approach becomes three lockfile commands and adds the movement
      table. That table includes `linux-raw-sys` 0.11.0 → 0.12.1, a 0.x minor
      (semver-incompatible) bump that `rustix` 1.1.6 requires; it was shown
      to the Don at approval.
    - Ask First adds the `syn` 3.0.9 exception and the `cargo deny` clause.
      Read with the approved table, the listed `linux-raw-sys` move is
      approved movement, not an Ask First trigger.
    - The Never rule now reads "an advisory" instead of "the advisory".
    - The matrix adds the `syn` row and the `windows-sys` edge row (the rule
      is adopted from the h2 precedent), and widens the allowed-warnings row.
  - The completed `rustls-webpki` change is kept, approved on 2026-10-26
    (`docs/governance/decisions/2026-10-26-don-approval-rustsec-2026-0382.md`).
  - KEEP: lockfile-only; no ignore entries.

## Verification

**Commands** (reproducible from the baseline `ce731fa`):
- `cargo update -p rustls-webpki --precise 0.103.16`
- `cargo update -p anyhow --precise 1.0.104`,
  `cargo update -p serde_json --precise 1.0.154`,
  `cargo update -p thiserror --precise 2.0.21` and
  `cargo update -p tempfile --precise 3.27.0` -- expected: only the listed
  packages move.
- Restore any `windows-sys` edge that re-resolution re-pointed to the
  baseline 0.61.2. Then `git diff ce731fa -- Cargo.lock | grep -c windows-sys`
  must print 0, and `Cargo.lock` must hash to `c457e98f…`.
- `cargo audit` -- expected: no vulnerabilities.
- `cargo metadata --locked --format-version 1` -- expected: success.
- `bash scripts/verify-bmad.sh` -- expected: pass.

### Verification record, 2026-11-04

**Toolchain:**
- `rustc 1.92.0 (ded5c06cf 2025-12-08)` and `cargo 1.92.0 (344c4567c
  2025-10-21)` from `rust-toolchain.toml`, host `x86_64-apple-darwin`.
- The tools match the CI pins: `cargo-audit 0.22.2`, `cargo-deny 0.18.5`,
  `mdbook v0.4.52`.
- The BMAD verifier ran on Python 3.10.0.
- The RustSec advisory database was at commit `965bf9ad083b`, with 1403
  advisories.

**Lockfile:**
- The `cargo update` commands moved exactly the packages in the Approach
  table.
- Re-resolution also re-pointed eight `windows-sys` edges from 0.61.2 to
  0.60.2 or 0.52.0: `anstyle-query`, `anstyle-wincon`, `errno`,
  `is-terminal`, `nu-ansi-term`, `rustix`, `tempfile` and `winapi-util`.
- The manifests of `rustix` 1.1.6 and `tempfile` 3.27.0 declare
  `windows-sys = ">=0.52, <0.62"`, so neither requires the new edge. All eight
  edges were restored to the baseline 0.61.2 by a script that rewrote only
  `windows-sys` references, so the committed lockfile is Cargo output plus
  that edit. A later `cargo update` or Dependabot run is likely to re-point
  them again; that is harmless, since both versions are in range.
- Every other `syn` reference now reads `syn 2.0.116`. Cargo adds the version
  to tell the two locked majors apart. The `syn` 2.x entry itself is
  unchanged.
- `cargo metadata --locked --format-version 1` exited 0 and left the
  lockfile bytes unchanged.
- The final `Cargo.lock` SHA-256 is
  `c457e98fb5b6c96df6bde29a17040d97a0c74a411a824ee0b2de6275547cc80a`.

**Package movement against the baseline `ce731fa`, 310 to 311 packages:**

| Package | Before | After |
|---|---|---|
| `rustls-webpki` | 0.103.15 | 0.103.16 |
| `anyhow` | 1.0.103 | 1.0.104 |
| `serde_json` | 1.0.149 | 1.0.154 |
| `thiserror` | 2.0.18 | 2.0.21 |
| `thiserror-impl` | 2.0.18 | 2.0.21 |
| `tempfile` | 3.25.0 | 3.27.0 |
| `rustix` | 1.1.3 | 1.1.6 |
| `linux-raw-sys` | 0.11.0 | 0.12.1 |
| `syn` | 2.0.116 | 2.0.116, plus 3.0.9 added |

No other package was added or removed. Apart from the `syn` version labels,
no unchanged package's dependency list changed.

**`cargo audit`:**
- Before, on the baseline `ce731fa` lockfile: exit 1, with one vulnerability
  (RUSTSEC-2026-0382 in `rustls-webpki` 0.103.15) and one allowed warning.
- After: exit 0 over 311 crates, with no vulnerability. One allowed warning
  remains: `tokio` 1.49.0 RUSTSEC-2026-0386 (unsound). `pin-utils` was not
  listed.

**Proof list:** every command below exited 0.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass, no warnings |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | pass |
| `bash scripts/check-release-portability.sh` | pass |
| `bash scripts/test-build-pages.sh` | Pages artifact contract verified |
| `bash scripts/build-pages.sh` | pass |
| `bash scripts/test-verify-bmad.sh` | fixture matrix passed |
| `BMAD_RENDERED_OUTPUT=target/pages bash scripts/verify-bmad.sh` | pass |
| `bash scripts/test-publish-crates.sh` | pass, against stubbed `cargo` and `curl` |
| `cargo package --workspace --allow-dirty --no-verify --locked` | pass |
| `cargo audit` | no vulnerabilities |
| `cargo deny check licenses sources` | `licenses ok, sources ok` |
| `cargo test --workspace --all-targets --locked` | 275 passed, 0 failed, in 32 test binaries |
| `forgetest validate --suite eval-suites/rust-agent-v1/suite.toml --calibrate` | 12 of 12 tasks calibrated |
| `bash scripts/verify-sample-reports.sh` | contract verified |
| `forgetest demo --runner local --format all` | pass |
| `forgetest demo --mode repository --runner local --format all` | pass |
| `cargo install --path crates/forgetest-cli --locked`, then `--version` and `validate` | pass |

Both demos wrote to a temporary directory outside the repository rather than
`./proof`, so the working tree stayed clean.

**Linux cross-check:**
- On macOS, `rustix` does not compile `linux-raw-sys`. To cover it,
  `cargo test --workspace --all-targets --locked` also ran on Linux
  (`aarch64`) in `rust:1.92.0-bookworm`, the verifier image's base.
- The repository was mounted read-only, with a tmpfs at `target/`.
- Result: 275 passed and 0 failed, in 32 test binaries.
- That run compiled `rustix` 1.1.6, `linux-raw-sys` 0.12.1, `syn` 3.0.9,
  `tempfile` 3.27.0, `thiserror-impl` 2.0.21 and `rustls-webpki` 0.103.16.

**Further checks, 2026-11-04:**
- **TLS smoke on `rustls-webpki` 0.103.16:** the legacy snippet path ran with
  invalid dummy keys against the real default endpoints.
  - `api.anthropic.com` answered `authentication_error: invalid x-api-key`.
  - `api.openai.com` answered `Incorrect API key provided`.
  - So the TLS handshake and certificate verification succeeded on both, and
    nothing was billed. No test in the suite performs a TLS handshake, since
    the provider tests use plain-HTTP mocks. That gap ends when S6 removes
    the legacy providers.
- **Minimum Rust versions** of the new versions, all at or below the
  workspace's 1.92:

  | Crate | `rust-version` |
  |---|---|
  | `tempfile` 3.27.0 | 1.63 |
  | `rustix` 1.1.6 | 1.65 |
  | `linux-raw-sys` 0.12.1 | 1.63 |
  | `syn` 3.0.9 | 1.71 |
  | `thiserror` and `thiserror-impl` 2.0.21 | 1.77 |
  | `serde_json` 1.0.154 | 1.71 |
  | `anyhow` 1.0.104 | 1.68 |
  | `rustls-webpki` 0.103.16 | 1.71 |

- **`syn` 3.0.9 provenance:** published on crates.io on 2026-10-12 by
  `dtolnay`, who also maintains `thiserror`.
- **`cargo deny` scope:** CI runs `cargo deny check licenses sources` only.
  Duplicate majors (`bans`) and advisories are not CI gates; advisories are
  covered by `cargo audit`.
- **Protected studies and source identities:** `verify-bmad.sh` checks both
  the protected-study tree identities and the authoritative-source
  identities in `_bmad-output/project-context.md`, and it passed after every
  edit.
- **Architectures covered locally:** macOS (`x86_64-apple-darwin` toolchain)
  and Linux `aarch64`. Linux `x86_64` and Windows are covered by CI.

**Not run locally; left to pull-request CI:**
- the ARM64 `cross` release build;
- the Docker proof job: the image build, the gated Docker tests and the Docker
  demos;
- the Windows test matrix.

**Scope:**
- `git diff --name-only ce731fa` lists `Cargo.lock`, this spec and
  `_bmad-output/index.yaml`. The 2026-11-04 decision record is untracked.
- No manifest, `rust-toolchain.toml`, `deny.toml` or `studies/` path changed.

## Suggested Review Order

**The security fix**

- Entry point: `rustls-webpki` 0.103.16 closes RUSTSEC-2026-0382; it is the first commit.
  [`Cargo.lock:1739`](../../Cargo.lock#L1739)

- TLS smoke on both legacy endpoints shows certificate verification still works.
  [`spec-rustls-webpki-rustsec-2026-0382.md:250`](spec-rustls-webpki-rustsec-2026-0382.md#L250)

**The batched bumps**

- `syn` 3.0.9 locked alongside 2.x, required by `thiserror-impl` 2.0.21.
  [`Cargo.lock:1966`](../../Cargo.lock#L1966)

- `thiserror-impl` 2.0.21, the only consumer of `syn` 3.
  [`Cargo.lock:2046`](../../Cargo.lock#L2046)

- `tempfile` 3.27.0 pulls `rustix` 1.1.6 and `linux-raw-sys` 0.12.1.
  [`Cargo.lock:2018`](../../Cargo.lock#L2018)

- `linux-raw-sys` 0.12.1 is the 0.x minor bump that `rustix` requires.
  [`Cargo.lock:1218`](../../Cargo.lock#L1218)

**Governance**

- Frozen changes and re-approval, including the `linux-raw-sys` reading.
  [`spec-rustls-webpki-rustsec-2026-0382.md:119`](spec-rustls-webpki-rustsec-2026-0382.md#L119)

- Verification record: movement table, `windows-sys` edge restore, audit before and after.
  [`spec-rustls-webpki-rustsec-2026-0382.md:160`](spec-rustls-webpki-rustsec-2026-0382.md#L160)

- Re-plan decisions and the unconfirmed S2 key revocation, with its deadline.
  [`2026-11-04-don-decisions-replan.md:69`](../../docs/governance/decisions/2026-11-04-don-decisions-replan.md#L69)
