---
schema_version: 1
ownership: human-owned
locked_baseline_revision: 88385f60adb116a04fc3834152c6d40f7b8fb916
digest:
  algorithm: SHA-256
  encoding: lowercase-hex
  byte_semantics: exact-file-bytes
review:
  reviewed_at: "2026-08-29T12:15:33Z"
  reviewer_identity: repository-don-001
  reviewer_identity_type: repository-local-alias
  reviewer_role: Don
  approval_basis: Don-approved 2026-08-29 BMAD repository setup specification review
source_identities:
  Cargo.toml: a6e7ddd126a79d550372e54efbbe3e8dd343070b51aa826aa10c873dd277292d
  ARCHITECTURE.md: a519140ed33cc0c191bc418ed719bf884f306c551e51c88488499c35f4c97dc2
  SECURITY.md: 7e122955d9f3a0615f856de3e7580da37738e23b434f5c87707b0aedaabb056a
  studies/rust-agent-v1/README.md: 85b7b6cb48559a7da5aef61bdaab8d45fd0726893b3f884afe268312590b2620
  studies/rust-agent-pilot/README.md: 6095b6e507b6acbf2261630b1639abfc3d77ffed26581704025ad9d7bcb8ede9
  docs/book.toml: 9139b870f6941f36c0f9f6dd7fc8d808cff399f2be00a021e77d207a47274ed4
  docs/src/SUMMARY.md: eac5289fd4fa3089f8e7bf7c6847273141350add2a76e5de102a4587c3c0f234
  .github/workflows/ci.yml: 3ec5cb6452290cd608adc2e76d9984b26e73557776a7f9a2b9002de4433ba729
protected_study_tree_identities:
  algorithm: sha256-normalized-file-manifest-v1
  roots:
    studies/rust-agent-v1:
      file_count: 3
      tree_sha256: 9911f42520223b33a1f06429d203be828680a496bf21f618a1b8dc82047da78a
    studies/rust-agent-pilot:
      file_count: 2
      tree_sha256: 873618af69b8fa7a2eb7893fd510112dbdef444f865e477bf4b72b8b73a3c17f
ci_proof_contract:
  workflow: .github/workflows/ci.yml
  job_count: 5
  step_count: 34
  jobs:
    - id: quality
      name: Quality and supply chain
      steps:
        - Check out source
        - Cache Cargo state
        - Check formatting
        - Deny Clippy warnings
        - Deny rustdoc warnings
        - Check release portability
        - Build and verify Pages artifact
        - Test resumable crate publisher
        - Dry-run workspace packages
        - Audit RustSec advisories
        - Audit licenses and sources
    - id: test
      name: "Test (${{ matrix.os }})"
      steps:
        - Check out source
        - Cache Cargo state
        - Run all targets
    - id: release-portability
      name: ARM64 Linux release build
      steps:
        - Check out source
        - Cache Cargo state
        - Check native dependency portability
        - Install cross
        - Cross-build release binary
    - id: offline-proof
      name: Deterministic local proof
      steps:
        - Check out source
        - Cache Cargo state
        - Calibrate all 12 repository tasks
        - Verify committed sample-report contract
        - Run no-key snippet demo
        - Run no-key repository-agent demo
        - Smoke-test installed binary
        - Upload deterministic evidence
    - id: docker-proof
      name: Hardened Docker proof
      steps:
        - Check out source
        - Cache Cargo state
        - Build pinned verifier image
        - Run gated Docker integration tests
        - Run no-key Docker snippet demo
        - Run no-key Docker repository-agent demo
        - Upload Docker evidence
---

# BMAD project context

This is a human-owned map of existing repository authority for planning and
implementation. Framework regeneration must not overwrite it. The recorded
source identities are SHA-256 digests of each source's exact bytes, encoded as
64 lowercase hexadecimal characters. At the review above, every recorded source
matched the same path at locked baseline revision
`88385f60adb116a04fc3834152c6d40f7b8fb916`.

Protected-study tree identities use `sha256-normalized-file-manifest-v1`: hash
each regular file's exact bytes, map its study-relative POSIX path to that
lowercase SHA-256 value, serialize the map as recursively key-sorted compact
UTF-8 JSON plus LF, and SHA-256 the result. Every discovered frozen, locked, or
pre-registered protocol root must have a matching full-tree record.

BMAD launcher rendering requires `uv` and follows the renderer's PEP 723
`requires-python = ">=3.11"` declaration. The repository BMAD verifier is
separate and requires Python >=3.10. `scripts/test-bmad-render.sh` exercises the
tracked Codex `bmad-build` launcher without running installer or regeneration.
The 2026-08-29 review retains the approved repository-local alias authority
model; its digests record byte provenance and do not create an external or new
cryptographic identity binding.

## Authority and conflict handling

The Cargo workspace, architecture, security policy, protected-study protocols,
mdBook sources, and CI workflow are authoritative over generated BMAD guidance.
BMAD coordinates work; it does not redefine those sources. Apply each authority
within its documented scope. If authoritative sources appear to conflict, stop
and obtain repository-maintainer review rather than using generated guidance to
choose between them.

- Workspace membership and shared package boundaries: [`Cargo.toml`](../Cargo.toml)
- Architectural boundaries and invariants: [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- Security, disclosure, and sensitive-data rules: [`SECURITY.md`](../SECURITY.md)
- Protected study protocols: [`studies/rust-agent-v1/README.md`](../studies/rust-agent-v1/README.md)
  and [`studies/rust-agent-pilot/README.md`](../studies/rust-agent-pilot/README.md)
- Public documentation source and navigation: [`docs/book.toml`](../docs/book.toml)
  and [`docs/src/SUMMARY.md`](../docs/src/SUMMARY.md)
- Required proof gates: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)

## Workspace boundaries

The workspace contains exactly the six members declared in `Cargo.toml`:

- `crates/forgetest-core`
- `crates/forgetest-runner`
- `crates/forgetest-providers`
- `crates/forgetest-report`
- `crates/forgetest-agents`
- `crates/forgetest-cli`

Follow `ARCHITECTURE.md` for their responsibilities and dependency direction,
and follow `SECURITY.md` for trust boundaries and handling requirements. Link to
those sources in planning artifacts instead of copying their normative rules.

## Protected studies

BMAD workflows must not mutate or use either `studies/rust-agent-v1` or
`studies/rust-agent-pilot` as scratch space. This protection applies
automatically to every future study whose protocol declares itself frozen,
locked, or pre-registered; no separate allowlist update is required.

A change to any protected task, grader, prompt, reference patch, model, effort,
image, budget, retry policy, endpoint, publication gate, or other frozen design
term requires a new study. Do not reinterpret planning or implementation work as
permission to edit an existing protected study.

## Public documentation boundary

`docs/book.toml` selects `docs/src` as the public mdBook source, and
`docs/src/SUMMARY.md` controls its navigation. `_bmad` and `_bmad-output` are
internal by default: do not add them to mdBook navigation, link them from public
mdBook sources, copy them during a documentation build, or include them in
rendered public output without an explicit reviewed publication decision.

## CI proof contract

The `ci_proof_contract` front matter records all five baseline job IDs and all
34 named steps from `.github/workflows/ci.yml`, grouped by job. It is supporting
inventory, not a substitute for the workflow itself. `bmad.lock.json` provides
the corresponding mechanical baseline inventory; generated BMAD guidance and
lock metadata do not override the CI workflow. Any authoritative-source change
requires an explicit review and refreshed exact-byte identity here before BMAD
verification may pass.
