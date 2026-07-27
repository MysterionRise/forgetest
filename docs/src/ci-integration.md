# CI Evidence

The checked-in `.github/workflows/ci.yml` defines four proof jobs.

## Quality and Supply Chain

- `cargo fmt --all -- --check`
- All-target Clippy with warnings denied.
- Rustdoc with warnings denied.
- mdBook build.
- Workspace package dry run.
- RustSec advisory scan.
- License and source policy scan.

## Cross-Platform Tests

`cargo test --workspace --all-targets --locked` runs on Linux, macOS, and
Windows with Rust 1.92.0 from `rust-toolchain.toml`.

## Deterministic Local Proof

The Linux job:

1. Calibrates all 12 repository tasks.
2. Verifies the checked-in sample-report schema and provenance contract.
3. Runs the no-key local snippet demo.
4. Runs the no-key local repository-agent demo.
5. Installs the binary into an isolated prefix and smoke-tests it.
6. Uploads the generated evidence.

## Hardened Docker Proof

The Docker job:

1. Builds the pinned verifier image and locked offline cache.
2. Runs the gated Docker execution integration test.
3. Runs the no-key Docker snippet demo.
4. Runs the no-key repository demo with Docker grading.
5. Uploads private and redacted evidence bundles.

A claim is CI-proven only for a commit with a green workflow run. The presence
of workflow YAML alone is not evidence that a particular commit passed.

## Paid Agent Studies

Do not put an unlocked paid-model benchmark on every pull request. Use a
manual, access-controlled workflow or disposable runner:

1. Review and calibrate the suite.
2. Build and publish immutable agent/verifier images.
3. Create `benchmark.lock.toml`.
4. Run the exact locked command.
5. Retain raw artifacts privately.
6. Review the redacted bundle.
7. Publish the dated protocol, lock, public evidence, and workflow URL.

The planned v1 public study is 12 tasks, 2 agents, and 3 trials. It is not a
general leaderboard and should not be described as one.

## SARIF

SARIF is intentionally restricted to deterministic compiler, Clippy, and
grader findings. Free-form agent messages are not code-scanning findings.

Upload `public/report.sarif` only after redaction review.

## Tagged Releases

`.github/workflows/release.yml` reruns the release quality gate, builds
cross-platform archives, publishes the verifier image to GHCR, records its
immutable digest, generates per-crate CycloneDX JSON SBOMs, writes
`SHA256SUMS`, and creates GitHub provenance attestations for both the image and
release assets. It then publishes crates in dependency order, including
`forgetest-agents`.

These are workflow guarantees, not claims about an unreleased tag. Verify a
published asset with `gh attestation verify` against this repository.
