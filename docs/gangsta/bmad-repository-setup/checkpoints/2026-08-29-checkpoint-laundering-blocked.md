---
heist: bmad-repository-setup
phase: laundering
status: blocked
timestamp: 2026-08-29T20:53:40Z
next-action: Obtain Don approval for a separate dependency-change plan addressing RUSTSEC-2026-0258, then rerun every acceptance gate.
completed-wps: [WP-001, WP-002, WP-003, WP-004, WP-005, WP-006, WP-007, WP-008, WP-009, WP-010, WP-011, WP-012, WP-013]
blocked-wps: [WP-014]
failed-wps: []
technical-debt-audit:
  copy-paste: SKIPPED - jscpd is not installed
  long-files: PASS - reviewed production files at 51-454 lines and the 465-line harness
  test-coverage: SKIPPED - the repository has no configured coverage tool or threshold
  clean-architecture: PASS
  clean-code: PASS
  documented-exceptions: none
artifacts:
  - _bmad-output/implementation-artifacts/spec-bmad-repository-setup.md
  - _bmad-output/implementation-artifacts/deferred-work.md
  - docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md
---

# Laundering Checkpoint: Blocked on Dependency Audit

## Outcome

The repo-wide BMAD 6.11.0, BMM-only setup and its governance verifier are
implemented within the approved contract. The final context-free inspector
found no reproducible Critical or Important issue in the accepted scope after
independently exercising the focused harness and real verifier. The work cannot
be declared complete because the signed final work package requires every
existing audit gate to pass, and the unchanged dependency graph fails the
current RustSec audit.

No dependency, Cargo product, protected-study, or public-mdBook source was
changed. No commit, push, or pull request was created.

## Verification Evidence

| Gate | Result |
| --- | --- |
| BMAD fixture and adversarial harness | PASS: 211/211 assertions |
| Real checkout BMAD verifier | PASS, including a freshly built `target/pages` rendered-output scan |
| Runtime and static checks | PASS: Bash 3.2 syntax, explicit Python 3.10 execution, Ruff, PLR2004, and all verifier/test files below 500 physical lines |
| BMAD launcher render smoke | PASS with the immutable tracked generation |
| Rust workspace checks | PASS: formatting, all-target Clippy, all-target tests, and rustdoc |
| Release and distribution checks | PASS: portability, package dry-run, mdBook/Pages, publisher, sample reports, local and Docker demos, pinned image, and ARM64 cross-build |
| Installation parity | PASS: 49 skills and 243 files in each launcher root |
| Disposable regeneration proof | PASS: 715 paths, 211 directories, and 504 generated files matched; seven human-owned files were preserved |
| Change boundaries and whitespace | PASS: Cargo/product, protected-study, and public-mdBook boundaries are unchanged; diff whitespace is clean |
| Final independent inspection | PASS: no reproducible Critical or Important finding |
| RustSec audit | BLOCKED: `h2` 0.4.13 is affected by `RUSTSEC-2026-0258`; remediation requires 0.4.16 or newer. The existing allowed warning for yanked `chacha20` 0.10.1 remains. |

## Integrity Identities

- Current CI workflow SHA-256:
  `3ec5cb6452290cd608adc2e76d9984b26e73557776a7f9a2b9002de4433ba729`.
- CI workflow after exact marker removal:
  `32f4bbe521b200cae48cb32821713318a8d9d04d37434a166e1018fcc4bd63f0`.
- Preserved baseline Pages helper SHA-256:
  `145f33a8cc418fdf85c63eecf0bb0e54ce34e1c8f2c1b2236f2bd76268301b92`.
- Byte-identical launcher tree SHA-256:
  `0164e21e498599c619505ea64e76ca2f81ca4a4b1ac3abdad94af9a99c0a48cb`.

## Technical-Debt Audit

| Check | Result |
| --- | --- |
| Copy/paste detection | SKIPPED: `jscpd` is unavailable locally; no exception was granted. |
| Long files | PASS: every production verifier is 51-454 physical lines and the largest test harness is 465; files in the 301-500 band were reviewed for single responsibility. |
| Test coverage | SKIPPED: no repository coverage tooling or threshold is configured; functional verification is represented by the 211-assertion harness and existing Rust suites. |
| Clean Architecture | PASS: the standard-library verifier is repository tooling with one coordinator and focused validation modules; no product layering changed. |
| Clean Code | PASS: lint, named-constant, nesting, stable-diagnostic, and debug-scaffolding checks passed. |
| Exceptions | None. The RustSec result is an unresolved blocker, not an accepted exception. |

## Resume Context

The approved scope prohibits dependency changes without a new approved plan.
Resume by obtaining that authority, planning the locked `h2` remediation and
the yanked `chacha20` investigation, applying the dependency change separately,
and rerunning the complete acceptance suite. Until then, keep the BMAD spec
`in-review`, the package audit `blocked`, and WP-014 open.
