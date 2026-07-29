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
release assets. When the protected `crates-io` environment contains a non-empty
`CARGO_REGISTRY_TOKEN`, it then publishes crates in dependency order, including
`forgetest-agents`. Without that credential, the workflow reports the registry
channel as disabled and leaves the GitHub release complete.

These are workflow guarantees, not claims about an unreleased tag. Verify a
published asset with `gh attestation verify` against this repository.

The release matrix also executes each native binary before archiving it.
Cached `cross` and `cargo-cyclonedx` installations are version-checked and
reinstalled when stale, so a cache hit cannot silently select another tool
version.

Crates are published in dependency order through `scripts/publish-crates.sh`.
The publisher independently checks the exact crates.io version and archive
checksum, skips only an identical already-visible package, and waits for each
new package to become visible. A release job can therefore resume after a
partial registry publication without accepting mismatched content. The
credential gate is outside this script, so an enabled publisher still fails
closed on authentication and registry errors.

## Evidence Site

`.github/workflows/pages.yml` builds the mdBook plus the committed
publication-safe sample reports with `scripts/build-pages.sh`, uploads one
Pages artifact, and deploys it through the protected `github-pages`
environment. CI runs
`scripts/test-build-pages.sh` to assert the expected pages and report files
before deployment. The Pages build independently verifies artifact manifests
and scans public reports for private host paths and credential patterns.

A repository administrator must select **GitHub Actions** as the Pages source
once before the first deployment. The workflow intentionally uses only the
scoped `GITHUB_TOKEN`, which cannot enable Pages on its own.
