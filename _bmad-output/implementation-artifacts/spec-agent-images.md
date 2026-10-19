---
title: 'Agent images: pinned Codex CLI and Claude Code images for the benchmark profile'
type: 'feature'
created: '2026-10-18'
status: 'done'
baseline_commit: 'c4db5941a0fa6cf695d16b95fa8c6635e7a9dc9f'
review_loop_iteration: 0
context:
  - '{project-root}/docker/agents/README.md'
  - '{project-root}/_bmad-output/planning-artifacts/forgetest-roadmap-2026-10.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The benchmark profile only runs agents from immutable `@sha256`
images. None exist yet (`docker/agents/` contains only the contract README), so
`agents lock`, benchmark runs and pilot-r2 are all blocked.

**Approach:**
- Add build recipes for amd64 Codex CLI 0.163.0 and Claude Code 2.1.302 images.
  - Each recipe installs the vendor's **native binary**, checksum-verified, on a
    digest-pinned Rust 1.92 base, with telemetry and the auto-updater off.
- Add a build script that pushes the images to a registry the operator controls
  (a local `registry:2` by default) and prints `@sha256` references.
- Prove the images with forgetest's own credential-free preflight and lock.
- Make no Rust product changes and no CLI-flag changes; flags belong to S3.

## Boundaries & Constraints

**Always:**
- Build for linux/amd64 only.
- Pin base images by digest.
- Download vendor binaries from official release locations and verify them
  against vendor-published checksums before installing. Record each source URL
  next to its pin.
- Images contain no credentials, no provider config and no API keys.
- Images satisfy `docker/agents/README.md`.
- Ship recipes only. forgetest never publishes vendor CLI images.

**Ask First:**
- A vendor binary has no published checksum.
- A CLI turns out to need Node, a writable root, or an entrypoint.
- Pushing to any non-local registry.
- Any Rust source change outside the gated test.
- Changing the image contract beyond documenting the recipes.

**Never:**
- Edit built-in profile flags or `.github/workflows/ci.yml`.
- Touch anything under `studies/`.
- Record an npm launcher as the executable.
- Bake keys or provider endpoints into images.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Build | `scripts/build-agent-images.sh` with the default registry `localhost:5001` | Verifier, Codex and Claude images are built for linux/amd64 and pushed; three `@sha256` references printed | Starts a pinned `registry:2` if none is running |
| Checksum mismatch | A tampered or changed download | Build fails before install and names the artifact | Non-zero exit |
| Preflight | Image run the way `doctor_container` runs it (UID 65532, no network, read-only, `noexec` `/tmp`, no env) | Prints the binary path, the native binary's SHA-256, and version `0.163.0` / `2.1.302` | N/A |
| Runtime contract | Arbitrary UID, `HOME=/tmp/home` not pre-created, read-only root | `codex --version` and `claude --version` exit 0 | Ask First if the CLI needs more |
| Remote registry | `FORGETEST_IMAGE_REGISTRY` is not local | Refuses unless `--allow-remote` is passed | Non-zero exit |

</frozen-after-approval>

## Code Map

- `docker/agents/README.md` -- the image contract:
  - non-root with an arbitrary UID;
  - `sh` and `sha256sum` on PATH;
  - read-only root;
  - `HOME=/tmp/home` at runtime;
  - `@sha256` references;
  - no redistribution of vendor images.
- `docker/forgetest-runner-rust.Dockerfile` -- the verifier base pattern
  (`rust:1.92.0-bookworm`, offline crate cache). Agent images reuse that base,
  pinned by digest.
- `crates/forgetest-agents/src/lib.rs` (read-only for this spec):
  - 596-641: agent `docker_arguments` (`--read-only`, `--cap-drop ALL`,
    `no-new-privileges`, tmpfs `/tmp`, `HOME=/tmp/home`, workspace UID).
  - 646-733: `docker_preflight_arguments` and `doctor_container`.
  - 882-895: the host `docker` runs with a cleared env and a temporary HOME, so
    images must already be present locally.
- `crates/forgetest-cli/src/commands/agents.rs`:
  - 77-239: `agents lock` (doctor, then record `cli_version` and
    `executable_sha256`).
  - 282-292: parses `NAME/MODEL=IMAGE`, splitting at the first `/`, so slugs
    work.
- `crates/forgetest-agents/tests/adapters.rs:215` -- the existing gated Docker
  test pattern (`FORGETEST_DOCKER_TEST`).
- `.github/workflows/release.yml:152-214` -- the pattern for capturing
  `RepoDigests`.

## Tasks & Acceptance

**Execution:**
- [x] `docker/agents/codex.Dockerfile`
  - Digest-pinned `rust:1.92.0-bookworm` base.
  - Download the official Codex 0.163.0
    `codex-package-x86_64-unknown-linux-musl.tar.gz` and verify it against the
    vendor's `codex-package_SHA256SUMS`.
  - Extract the whole package, unmodified, to `/opt/codex-0.163.0`, and verify
    `bin/codex` against its pinned SHA-256.
  - Symlink `/usr/local/bin/codex` to `/opt/codex-0.163.0/bin/codex`, so the
    CLI finds its packaged helpers (`codex-code-mode-host`, `codex-path/rg`,
    `codex-resources/bwrap`).
  - No ENTRYPOINT and no fixed USER.
- [x] `docker/agents/claude.Dockerfile`
  - Same base.
  - Install the official native linux-x64 Claude Code 2.1.302 binary, verified
    against the vendor manifest checksum.
  - `ENV DISABLE_AUTOUPDATER=1 DISABLE_TELEMETRY=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`.
- [x] `.dockerignore`
  - Exclude `target/`, `.git/`, `forgetest-results/`, `_bmad*/` and other
    build-irrelevant trees.
  - Keep `docker/runner-cache` available to the verifier build.
- [x] `scripts/build-agent-images.sh`
  - Uses buildx with `--platform linux/amd64`.
  - Builds the verifier and both agent images.
  - Pushes to `${FORGETEST_IMAGE_REGISTRY:-localhost:5001}`. If that local
    registry isn't running, it starts a digest-pinned `registry:2`.
  - Prints `NAME=REF@sha256:…` lines.
  - Refuses remote registries unless `--allow-remote` is passed.
- [x] `crates/forgetest-agents/tests/agent_images.rs` -- a test gated on
  `FORGETEST_AGENT_IMAGE_TEST=1`, with image refs supplied via env:
  - `doctor_container` reports the pinned version and native SHA-256.
  - A runtime smoke run with forgetest's agent Docker flags, an arbitrary UID
    and a missing HOME passes `--version`.
- [x] `docker/agents/README.md` -- document the recipes, pinned versions and
  checksum sources, the amd64-only rationale, the build script, the
  no-redistribution policy, and how to pass the refs to `agents lock`.
- [x] `CHANGELOG.md` -- add an Unreleased entry for the agent image recipes.

**Acceptance Criteria:**
- Given Docker with buildx, when the build script runs with the default local
  registry, then it prints three `@sha256` references. The images then run on
  this arm64 Mac through emulation.
- Given those references and **no API key in the environment**, when
  `forgetest agents lock --suite eval-suites/rust-agent-v1/pilot.toml --agent
  codex/gpt-6-luna=<ref> --agent claude/claude-haiku-5-5=<ref>
  --verifier-image <ref> --trials 1` runs, then it succeeds. The lock's
  `executable_sha256` equals the native binary SHA-256 for both agents.
- Given the gated env, when `cargo test -p forgetest-agents --test
  agent_images` runs, then the preflight and runtime-contract tests pass.
- Given the full CI command set, when it runs locally, then every step passes,
  and `ci.yml` and `studies/` are untouched.

## Design Notes

- **Native binaries** make `executable_sha256` cover the real agent instead of
  a JavaScript launcher, and remove Node from the image.
- **amd64 only** gives one lock that is valid on the GitHub runner that will
  execute pilot-r2 and, under emulation, on the maintainer's arm64 Mac.
- **The Rust toolchain is in the image** because agents must run `cargo test`
  inside `/work`.
- **Provider endpoints, web-tool flags and auth are deliberately absent.** S3
  adds them as digested profile arguments and environment.
- **Remote publishing** (a private GHCR package for the pilot runner) happens
  later, under the S5 plan.
- **`cli_version` is recorded from the preflight's combined output**, so it
  includes container stderr, such as Docker's platform warning on arm64 and
  Codex's PATH-alias warning. A lock made on arm64 therefore does not verify on
  amd64 until S3 records the version from stdout only. This spec makes no Rust
  change for it.

## Spec Change Log

- **2026-10-19, Don decisions at the step-3 Ask First gate.**
  - Triggering findings:
    - the vendor checksum file lists only the Codex package, not the
      standalone binary;
    - with only `bin/codex` installed, every `codex exec` reports
      "Code Mode is unavailable", and the vendor sandbox (`bwrap`) is absent;
    - macOS AirPlay Receiver holds port 5000;
    - `cli_version` includes container stderr.
  - Amended:
    - the Codex task now installs the full verified package (Don: "Full
      package as shipped");
    - the default registry is now `localhost:5001` in the frozen Build row and
      the build-script task (Don: "Default to localhost:5001", approving the
      frozen amendment);
    - the `cli_version` fix moves to S3 (Don: "Fold into S3").
  - Known-bad states avoided:
    - an image that lacks the vendor's helpers and sandbox;
    - a default that fails on the maintainer's Mac;
    - a Rust change outside this spec's approved scope.
  - KEEP:
    - the allowlist `.dockerignore`;
    - the Claude recipe;
    - `/etc/codex/config.toml`;
    - registry probing, the pull-back by digest, and the remote refusal;
    - the four gated tests;
    - the README structure.

## Verification

**Commands:**
- `bash scripts/build-agent-images.sh` -- expected: three `@sha256` lines.
- `env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY forgetest agents lock …
  --trials 1 --output <tmp>/lock.toml` -- expected: exit 0, with native SHAs in
  the lock.
- `FORGETEST_AGENT_IMAGE_TEST=1 FORGETEST_CODEX_IMAGE=<ref>
  FORGETEST_CLAUDE_IMAGE=<ref> cargo test -p forgetest-agents --test
  agent_images` -- expected: pass.
- The full local CI mirror -- expected: all steps pass.

## Suggested Review Order

**Vendor binaries, verified before install**

- Entry point: the verified Codex package checksum gates everything after it.
  [`codex.Dockerfile:36`](../../docker/agents/codex.Dockerfile#L36)

- Whole package extracted as shipped; `bin/codex` checked again against its own pin.
  [`codex.Dockerfile:40`](../../docker/agents/codex.Dockerfile#L40)

- Symlink keeps `/usr/local/bin/codex` stable while helpers resolve beside the real binary.
  [`codex.Dockerfile:48`](../../docker/agents/codex.Dockerfile#L48)

- Claude's native binary checked against the vendor manifest checksum.
  [`claude.Dockerfile:30`](../../docker/agents/claude.Dockerfile#L30)

**Updates and telemetry off**

- Codex has no env switches, so a system config applies even with `--ignore-user-config`.
  [`codex.Dockerfile:54`](../../docker/agents/codex.Dockerfile#L54)

- Claude's three switches as image ENV, covered by the image digest.
  [`claude.Dockerfile:37`](../../docker/agents/claude.Dockerfile#L37)

**Build script and registry boundary**

- Remote registries refused unless `--allow-remote`; local ports validated first.
  [`build-agent-images.sh:61`](../../scripts/build-agent-images.sh#L61)

- Tags come from recipe ARGs, so a version bump cannot leave stale tags.
  [`build-agent-images.sh:91`](../../scripts/build-agent-images.sh#L91)

- Wrong buildx driver fails before the long verifier build.
  [`build-agent-images.sh:108`](../../scripts/build-agent-images.sh#L108)

- Probe distinguishes "nothing listening" from "something else holds the port".
  [`build-agent-images.sh:121`](../../scripts/build-agent-images.sh#L121)

- Single-manifest push, then pull-back by digest for forgetest's cleared-env Docker.
  [`build-agent-images.sh:182`](../../scripts/build-agent-images.sh#L182)

- Build context reduced to the verifier's crate-cache manifest.
  [`.dockerignore:5`](../../.dockerignore#L5)

**Documentation and limits**

- Recipes, pins, checksum sources and what each digest covers.
  [`README.md:47`](../../docker/agents/README.md#L47)

- Known `cli_version` limitation, moved to S3 by the Don.
  [`README.md:198`](../../docker/agents/README.md#L198)

**Tests**

- Gate panics on unexpected values instead of silently skipping.
  [`agent_images.rs:35`](../../crates/forgetest-agents/tests/agent_images.rs#L35)

- Preflight: pinned path, native SHA-256 and version line.
  [`agent_images.rs:50`](../../crates/forgetest-agents/tests/agent_images.rs#L50)

- Package helpers resolve and run for a non-root UID.
  [`agent_images.rs:204`](../../crates/forgetest-agents/tests/agent_images.rs#L204)

- Update and telemetry settings observed inside each image.
  [`agent_images.rs:241`](../../crates/forgetest-agents/tests/agent_images.rs#L241)

- No entrypoint or fixed user, read from the image config.
  [`agent_images.rs:301`](../../crates/forgetest-agents/tests/agent_images.rs#L301)

- Stub-based script test: refusal, ports, conflicts, driver and output contract.
  [`test-build-agent-images.sh:119`](../../scripts/test-build-agent-images.sh#L119)

- Unreleased entries for the recipes and the build-context change.
  [`CHANGELOG.md:18`](../../CHANGELOG.md#L18)
