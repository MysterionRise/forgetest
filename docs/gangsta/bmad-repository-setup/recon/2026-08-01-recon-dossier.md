---
heist: bmad-repository-setup
date: 2026-08-01
status: approved
---

# Reconnaissance Dossier: BMAD Repository Setup

## Objective

Set up the existing `forgetest` repository with the current stable BMAD Method so future extensions can move through explicit analysis, planning, architecture, story creation, implementation, and verification workflows. The setup should preserve the repository's current architecture, security boundaries, study protocols, documentation publication rules, and CI proof contract rather than replacing them (`README.md:10-19`, `ARCHITECTURE.md:14-32`, `SECURITY.md:22-74`, `.github/workflows/ci.yml:16-96`).

## Codebase Overview

- This is an established Rust 2021 workspace, not a greenfield project. It contains six crates: `forgetest-core`, `forgetest-runner`, `forgetest-providers`, `forgetest-report`, `forgetest-agents`, and `forgetest-cli` (`Cargo.toml:1-19`, `Cargo.toml:68-74`).
- The root `ARCHITECTURE.md` defines crate responsibilities, core extension interfaces, repository-suite constraints, agent execution, verification, identity locking, outcomes, and evidence schemas; it is the architectural baseline future BMAD artifacts must reference (`ARCHITECTURE.md:14-48`, `ARCHITECTURE.md:50-73`, `ARCHITECTURE.md:83-134`, `ARCHITECTURE.md:136-170`).
- The main executable is the `forgetest` CLI in `crates/forgetest-cli/src/main.rs`, while library extension seams include `AgentExecutor`, `WorkspaceEnvironment`, `EventSink`, `Grader`, `LlmProvider`, and `CodeRunner` (`crates/forgetest-cli/Cargo.toml:14-23`, `crates/forgetest-core/src/agent.rs:132-178`, `crates/forgetest-core/src/traits.rs:16-27`, `crates/forgetest-core/src/traits.rs:87-98`).
- Public product documentation is an mdBook rooted at `docs/src`; its navigation is explicitly controlled by `docs/src/SUMMARY.md`. Internal BMAD working artifacts should therefore remain under `_bmad-output/` unless deliberately promoted into the public book (`docs/book.toml:1-12`, `docs/src/SUMMARY.md:1-11`, `scripts/build-pages.sh:28-34`).
- Existing study protocols are already binding specifications for benchmark execution and publication. A BMAD plan must not silently revise a locked task, grader, prompt, reference patch, model, effort, image, budget, or retry policy; such a revision creates a new study (`studies/rust-agent-v1/README.md:43-59`, `studies/rust-agent-v1/README.md:97-109`, `studies/rust-agent-pilot/README.md:21-56`, `studies/rust-agent-pilot/README.md:91-104`).
- No committed `AGENTS.md`, BMAD installation, general roadmap, ADR tree, or contributor planning hierarchy currently exists (recon commands: targeted searches for `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `_bmad`, `.bmad*`, roadmap, ADR, decision, and contributing files returned no matching paths).
- Local `.claude/` and `.codex/` directories exist but are ignored, so they are per-machine conveniences rather than portable repository policy (`.gitignore:15-20`; `git check-ignore -v` resolves both directories to those rules).

## Existing Test Coverage

- The survey found 15 Rust integration-test files with 132 integration-test attributes, plus 120 source-unit-test attributes and three shell contract-test scripts (recon command outputs from `rg --files` and `rg -n '#\\[(tokio::)?test\\]'`).
- Coverage spans agent adapters and locks, CLI workflows, end-to-end compilation and grading, repository lifecycle/reporting/suite safety/calibration, local and Docker graders, redaction, and report contracts (`crates/forgetest-agents/tests/adapters.rs:23`, `crates/forgetest-cli/tests/cli_tests.rs:15`, `crates/forgetest-cli/tests/e2e_pipeline.rs:54`, `crates/forgetest-core/tests/repository_engine.rs:280`, `crates/forgetest-runner/tests/repository_grader.rs:29`, `crates/forgetest-report/tests/repository_outputs.rs:78`).
- CI runs formatting, all-target Clippy, rustdoc, workspace tests on Linux/macOS/Windows, package dry-runs, security/license audits, corpus calibration, deterministic local proofs, Docker proofs, and installed-binary smoke checks (`.github/workflows/ci.yml:16-209`).
- The authoritative local verification commands are the stricter CI forms: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` (`README.md:329-347`, `.github/workflows/ci.yml:35-42`, `.github/workflows/ci.yml:95-96`).
- No tests were executed during reconnaissance; the current pass/fail baseline is therefore unknown. The checkout was clean on `master` before this dossier was created (`git status --porcelain=v1` produced no output; `git branch --show-current` returned `master`).

## Dependencies

- Rust is pinned to 1.92.0 with `rustfmt` and Clippy (`rust-toolchain.toml:1-3`). Workspace libraries and test dependencies are centralized in the root manifest (`Cargo.toml:21-74`).
- The local BMAD prerequisites are available: Node 20.18.0, npm/npx 10.8.2, Python 3.10 through 3.13, and uv 0.9.25 (recon command outputs from `node --version`, `npm --version`, `python3.10 --version` through `python3.13 --version`, and `uv --version`).
- BMAD's official installation guide requires Node 20.12+, Git, and a supported AI tool; the stable installer is `npx bmad-method install` and records installed module versions in `_bmad/_config/manifest.yaml` ([official installation guide](https://docs.bmad-method.org/how-to/install-bmad/)).
- BMAD v6 installs framework assets under `_bmad/`, generated artifacts under `_bmad-output/`, and Codex launchers under `.agents/skills/` ([official getting-started guide](https://docs.bmad-method.org/tutorials/getting-started/), [official BMAD changelog](https://github.com/bmad-code-org/BMAD-METHOD/blob/main/CHANGELOG.md)).
- The repository has no Node manifest, so the BMAD installer version is not currently repository-pinned. Reproducibility should be captured from `_bmad/_config/manifest.yaml` and documented rather than adding Node as a product build dependency (`Cargo.toml:1-74`; recon manifest inventory found Cargo manifests only).

## Relevant Ledger Entries

### Applicable Insights

- None. `docs/gangsta/insights/` did not exist at reconnaissance time (recon path inventory).

### Applicable Negative Constraints

- None from a Gangsta ledger. `docs/gangsta/fails/` and `docs/gangsta/constitution.md` did not exist at reconnaissance time (recon path inventory).
- Existing repository constraints remain binding: security boundaries in `SECURITY.md`, architectural responsibilities in `ARCHITECTURE.md`, study immutability in `studies/`, and the CI proof contract in `.github/workflows/ci.yml` (`SECURITY.md:22-150`, `ARCHITECTURE.md:14-170`, `studies/rust-agent-v1/README.md:43-59`, `.github/workflows/ci.yml:16-209`).

## Risks and Unknowns

- **Generated framework footprint:** installing BMAD will add `_bmad/`, `_bmad-output/`, and tool-specific launchers. The stable installer version should be recorded and the diff reviewed because BMAD evolves rapidly ([official BMAD repository](https://github.com/bmad-code-org/BMAD-METHOD), [official installation guide](https://docs.bmad-method.org/how-to/install-bmad/)).
- **Tool integration:** Codex is expected to use `.agents/skills/`; Claude Code integration may also be useful because the repository already exercises both CLIs, but `.claude/` is currently ignored (`README.md:25-27`, `.gitignore:15-20`, [official BMAD changelog](https://github.com/bmad-code-org/BMAD-METHOD/blob/main/CHANGELOG.md)). Exact supported tool IDs must be confirmed with the installer before a non-interactive install.
- **Source-of-truth collision:** generated BMAD architecture or context could drift from `ARCHITECTURE.md`, `SECURITY.md`, or the study protocols. The project context must designate those files as authoritative and require explicit updates when a feature changes them (`ARCHITECTURE.md:14-170`, `SECURITY.md:22-150`, `studies/rust-agent-v1/README.md:43-59`).
- **Publishing boundary:** placing planning artifacts under `docs/src` would expose them through mdBook if linked in `SUMMARY.md`. Keep working artifacts under `_bmad-output/` and promote only reviewed user-facing documentation (`docs/book.toml:5`, `docs/src/SUMMARY.md:1-11`, `scripts/build-pages.sh:28-34`).
- **Test module overlap:** the optional BMAD Test Architect module overlaps with an already extensive repository-specific proof system. Installing it now would increase generated surface without a demonstrated planning need (`.github/workflows/ci.yml:16-209`, `README.md:329-347`; [official BMAD testing reference](https://docs.bmad-method.org/reference/testing/)).

## Recommended Scope

1. Install the latest stable BMAD v6 core plus the BMM module, integrated with Codex and Claude Code if both tool IDs are supported; do not install optional BMB, CIS, GDS, or TEA modules yet ([official installation guide](https://docs.bmad-method.org/how-to/install-bmad/)).
2. Generate and commit BMAD's repository framework/configuration and tool launchers, while keeping machine-local permission settings ignored (`.gitignore:15-20`).
3. Create `_bmad-output/project-context.md` for this brownfield repository. It should encode crate boundaries, authoritative documents, Rust/CI commands, TDD expectations, security rules, documentation rules, and study-protocol immutability. BMAD recommends this context file for established projects ([official established-projects guide](https://docs.bmad-method.org/how-to/established-projects/), [official workflow map](https://docs.bmad-method.org/reference/workflow-map/)).
4. Create empty tracked planning and implementation artifact directories only if the installer does not do so, with short README files explaining their lifecycle and the boundary between internal artifacts and public mdBook content.
5. Add a concise repository-level onboarding section or tracked instruction file that points future contributors to `bmad-help`, `_bmad-output/project-context.md`, `ARCHITECTURE.md`, `SECURITY.md`, the relevant study protocol, and the CI proof commands.
6. Verify installation manifests, launcher discovery, internal file references, Git ignore behavior, and the existing Rust formatting/test contract. No product behavior changes are in scope.
