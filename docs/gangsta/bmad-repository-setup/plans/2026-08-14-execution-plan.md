---
heist: bmad-repository-setup
date: 2026-08-14
status: approved
total-work-packages: 14
territories: 3
estimated-total-budget: 62000
---

# Execution Plan: BMAD Repository Setup

## Selected Upstream Baseline

- Exact stable installer: `bmad-method` 6.11.0, resolved from the npm registry
  on 2026-08-14.
- Registry integrity: `sha512-xP+ZFPa8eA/xa9O9LSW7u8xq3/cVqEaCfdRHYwqti/JFm9+F8/Zmp+UwhHUMPAoBhyOCbDtwOJedCEnFTqqz0Q==`.
- Downloaded tarball SHA-512:
  `c4ff9914f6bc780ff16bd3bd2d25bbbbcc6adff715a846827dd447630aad8bf2459bdf85f3f666a7e53084750c3c0a018723826c3b7038979d0849c54eaab3d1`.
- Package scope: bundled core and BMM only; optional external modules are not
  selected.
- Tool IDs and current package targets: `codex` to `.agents/skills` and
  `claude-code` to `.claude/skills`, confirmed from the inspected 6.11.0
  `platform-codes.yaml`.
- The package declares Node 20.12 or newer and no preinstall, install, or
  postinstall lifecycle script. The declared prepare script and full dependency
  resolution remain subject to the disposable audit Work Package.

## Territories

### Territory: Bootstrap and Locking

**Crew Lead Domain:** Exact-package inspection, disposable generation, repository
installation, generated launchers, and immutable lock metadata.

**Files:** `bmad.lock.json`, `_bmad/**`, `.agents/skills/bmad-*/**`,
`.claude/skills/bmad-*/**`, `.gitignore`,
`docs/gangsta/bmad-repository-setup/evidence/**`

**Conventions:** Use only the exact inspected installer; never regenerate in the
working checkout before the disposable comparison passes; never track unrelated
Claude or Codex local state.

**Workers:** 1 sequential worker

**Budget:** 18000 tokens

### Territory: Governance and Context

**Crew Lead Domain:** Repository instructions, authority and artifact lifecycle,
brownfield context, workflow tiers, and contributor onboarding.

**Files:** `AGENTS.md`, `CLAUDE.md`, `README.md`,
`_bmad-output/project-context.md`, `_bmad-output/authorities.yaml`,
`_bmad-output/index.yaml`, `_bmad-output/planning-artifacts/**`,
`_bmad-output/implementation-artifacts/**`

**Conventions:** Link to authoritative sources rather than duplicating them;
protect both current studies and all future frozen, locked, or pre-registered
protocols; treat repository-visible content as non-confidential.

**Workers:** 1 sequential worker

**Budget:** 16000 tokens

### Territory: Verification and CI

**Crew Lead Domain:** Test-first BMAD governance validation, negative fixtures,
context and CI drift detection, launcher parity, publication safety, and CI
integration.

**Files:** `scripts/verify-bmad.sh`, `scripts/test-verify-bmad.sh`,
`scripts/fixtures/bmad/**`, `.github/workflows/ci.yml`

**Conventions:** Additive checks only; retain every baseline CI job and step;
tests must demonstrate each required failure mode before verifier behavior is
implemented.

**Workers:** 1 sequential worker

**Budget:** 28000 tokens

## Work Packages

**Prevention Guidance (all Work Packages):** Do NOT reproduce Gangsta-internal
spec identifiers (FR-xxx, NFR-xxx, WP-xxx, where xxx is one or more digits) in
any deliverable — source code, test files, code comments, or documentation.
These identifiers belong only in `docs/gangsta/`.

### WP-001: Create the Failing Governance Test Matrix

**Territory:** Verification and CI

**Contract Clause:** FR-010; AC-003–AC-011

**Files:**
- Create: `scripts/test-verify-bmad.sh`
- Create: `scripts/fixtures/bmad/**`

**Acceptance Criteria:**

1. Tests cover lock/manifest mismatch, generated-file drift, launcher divergence,
   unsafe paths, invalid authority or artifact records, prohibited study
   instructions, stale project context, disallowed tracked Claude state,
   sensitive content, and accidental publication.
2. The test suite fails for the expected missing-verifier reason before
   production verification is added.

**Verification:** `bash scripts/test-verify-bmad.sh`

**Budget:** 6000 tokens

**Dependencies:** None

### WP-002: Record Exact Package Provenance

**Territory:** Bootstrap and Locking

**Contract Clause:** FR-001, FR-002; NFR-001; AC-001, AC-002

**Files:**
- Create: `bmad.lock.json`
- Create: `docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md`

**Acceptance Criteria:**

1. The evidence records registry metadata, tarball hashes, license, executable
   entry point, lifecycle scripts, declared and resolved dependencies, Node/npm
   runtime, platform, generation environment, and baseline Git revision.
2. The lock records the same immutable identities without containing cache paths
   or local credentials.
3. Any non-patch installer version is rejected as outside this signed plan.

**Verification:** `jq -e '.installer.version == "6.11.0" and .installer.integrity and .baseline.revision and .runtime.node and .runtime.npm' bmad.lock.json`

**Budget:** 5000 tokens

**Dependencies:** None

### WP-003: Prove the Disposable Install and Path Inventory

**Territory:** Bootstrap and Locking

**Contract Clause:** FR-002, FR-003, FR-011; NFR-004; AC-002, AC-004, AC-012

**Files:**
- Modify: `bmad.lock.json`
- Modify: `docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md`

**Acceptance Criteria:**

1. The exact package is installed with BMM, Codex, and Claude Code only in a
   disposable clean clone using neutral shared configuration and
   `_bmad-output` paths.
2. The audit records the generated inventory and confirms current launcher roots
   `.agents/skills` and `.claude/skills`.
3. The lock records normalized generated-path and baseline-CI inventories.

**Verification:** `bash scripts/test-verify-bmad.sh --case disposable-inventory`

**Budget:** 5000 tokens

**Dependencies:** WP-001, WP-002

### WP-004: Install the Locked Framework and Shared Launchers

**Territory:** Bootstrap and Locking

**Contract Clause:** FR-001–FR-003; NFR-002, NFR-007; AC-003–AC-005

**Files:**
- Create: `_bmad/**`
- Create: `.agents/skills/bmad-*/**`
- Create: `.claude/skills/bmad-*/**`
- Modify: `.gitignore`
- Modify: `bmad.lock.json`

**Acceptance Criteria:**

1. Repository output matches the approved disposable inventory and exact lock.
2. BMM is the only selected module and both tools use the same installed module
   identity.
3. Anchored ignore exceptions expose only installer-generated BMAD skill
   directories under `.claude/skills`; local settings and all other `.claude`
   and `.codex` state remain ignored.
4. Generated-file identities are recorded after human-owned paths are excluded.

**Verification:** `bash scripts/test-verify-bmad.sh --case generated-install`

**Budget:** 3000 tokens

**Dependencies:** WP-003

### WP-005: Create the Brownfield Project Context

**Territory:** Governance and Context

**Contract Clause:** FR-005, FR-009; NFR-005, NFR-006; AC-007, AC-010

**Files:**
- Create: `_bmad-output/project-context.md`

**Acceptance Criteria:**

1. Context links to the six-crate workspace, architecture, security, both current
   study protocols, future protected-study rule, mdBook boundary, and complete CI
   proof contract.
2. It states repository-authority precedence and contains reviewed source
   digests tied to the locked baseline revision.
3. It contains no copied secrets, confidential evidence, or machine-local paths.

**Verification:** `bash scripts/test-verify-bmad.sh --case project-context`

**Budget:** 4000 tokens

**Dependencies:** WP-004

### WP-006: Establish Authority and Artifact Registries

**Territory:** Governance and Context

**Contract Clause:** FR-006, FR-007; NFR-003, NFR-008; AC-008, AC-009

**Files:**
- Create: `_bmad-output/authorities.yaml`
- Create: `_bmad-output/index.yaml`
- Create: `_bmad-output/planning-artifacts/README.md`
- Create: `_bmad-output/implementation-artifacts/README.md`

**Acceptance Criteria:**

1. The authority registry uses stable identities, roles, scopes, and Don-approved
   governance without assuming confidentiality from Git visibility.
2. The index schema defines type-and-scope effectiveness, draft approval nulls,
   approval evidence, parent links, supersession, archival authority, and
   classification.
3. Artifact directories document durable versus transient ownership and naming
   rules without embedding Gangsta identifiers.

**Verification:** `bash scripts/test-verify-bmad.sh --case artifact-governance`

**Budget:** 5000 tokens

**Dependencies:** WP-004

### WP-007: Add Repository-Wide Agent Instructions

**Territory:** Governance and Context

**Contract Clause:** FR-004, FR-008, FR-009; AC-006, AC-010

**Files:**
- Create: `AGENTS.md`
- Create: `CLAUDE.md`

**Acceptance Criteria:**

1. Both tools identify BMAD as the default for qualifying extension work and
   direct agents to the same context, index, authorities, verification, and
   authoritative sources.
2. Tier precedence is frozen-study prohibition, urgent security, full planning,
   then trivial exemption; classification remains human-reviewed.
3. Trivial and urgent records include deterministic approval and deadline rules.
4. Both instruction files prohibit modification or scratch use of protected
   studies and confidential material.

**Verification:** `bash scripts/test-verify-bmad.sh --case repository-instructions`

**Budget:** 4000 tokens

**Dependencies:** WP-005, WP-006

### WP-008: Add Contributor Onboarding

**Territory:** Governance and Context

**Contract Clause:** FR-004, FR-008, FR-011, FR-012; AC-006

**Files:**
- Modify: `README.md`

**Acceptance Criteria:**

1. A concise section explains the default BMAD entry point, workflow tiers,
   approval sources, artifact locations, verification command, update policy,
   and internal/public documentation boundary.
2. It states that minor and major BMAD updates need a new approved plan.
3. It links to shared files without publishing internal artifact contents.

**Verification:** `bash scripts/test-verify-bmad.sh --case onboarding`

**Budget:** 3000 tokens

**Dependencies:** WP-007

### WP-009: Implement Lock, Manifest, Hash, and Path Verification

**Territory:** Verification and CI

**Contract Clause:** FR-001, FR-010, FR-011; NFR-001, NFR-004, NFR-008; AC-001–AC-003, AC-011, AC-012

**Files:**
- Create: `scripts/verify-bmad.sh`

**Acceptance Criteria:**

1. Verification checks lock/manifest/runtime/baseline consistency, normalized
   generated hashes, and the exact allowed generated roots.
2. It rejects absolute paths, traversal, symlinks, missing or duplicate targets,
   and generated/human-owned overlap.
3. It works in shallow CI checkouts using the normalized baseline inventory.

**Verification:** `bash scripts/test-verify-bmad.sh --case lock-and-paths`

**Budget:** 6000 tokens

**Dependencies:** WP-001, WP-002, WP-004, WP-006

### WP-010: Implement Authority and Artifact Validation

**Territory:** Verification and CI

**Contract Clause:** FR-006, FR-007, FR-010; NFR-002, NFR-003; AC-008, AC-009

**Files:**
- Modify: `scripts/verify-bmad.sh`

**Acceptance Criteria:**

1. Verification enforces authority roles/scopes, approval eligibility and
   evidence, Don-controlled registry changes, draft nulls, type-and-scope
   effectiveness, parent/supersession links, and archival authority.
2. Only public and repository-visible classifications are accepted, and both are
   checked for prohibited sensitive content.

**Verification:** `bash scripts/test-verify-bmad.sh --case authority-and-artifacts`

**Budget:** 5000 tokens

**Dependencies:** WP-001, WP-006, WP-009

### WP-011: Implement Launcher, Study, and Publication Guards

**Territory:** Verification and CI

**Contract Clause:** FR-003, FR-009, FR-010, FR-012; NFR-002, NFR-007; AC-004, AC-005, AC-010, AC-011

**Files:**
- Modify: `scripts/verify-bmad.sh`

**Acceptance Criteria:**

1. Codex and Claude skills match by intended workflow identity, version, context
   reference, authority precedence, and study protection.
2. Only allowlisted BMAD Claude skills may be tracked; local settings remain
   ignored.
3. Negative fixtures protect both current studies and a future marked protocol.
4. No internal BMAD file is navigated, linked, copied, or rendered by mdBook.

**Verification:** `bash scripts/test-verify-bmad.sh --case launchers-studies-publication`

**Budget:** 5000 tokens

**Dependencies:** WP-001, WP-004, WP-005, WP-007, WP-009

### WP-012: Implement Context and CI Baseline Drift Checks

**Territory:** Verification and CI

**Contract Clause:** FR-005, FR-010; NFR-005, NFR-006; AC-007, AC-011, AC-014

**Files:**
- Modify: `scripts/verify-bmad.sh`
- Modify: `scripts/test-verify-bmad.sh`
- Modify: `scripts/fixtures/bmad/**`

**Acceptance Criteria:**

1. Source identity changes require an explicit project-context review update.
2. Every baseline CI job and step recorded in the lock remains present with
   unchanged normalized semantics.
3. Fixtures prove detection without requiring full Git history.

**Verification:** `bash scripts/test-verify-bmad.sh --case context-and-ci-drift`

**Budget:** 4000 tokens

**Dependencies:** WP-001, WP-002, WP-005, WP-009

### WP-013: Add the Additive BMAD CI Gate

**Territory:** Verification and CI

**Contract Clause:** FR-010; AD-006; AC-013, AC-014

**Files:**
- Modify: `.github/workflows/ci.yml`

**Acceptance Criteria:**

1. CI runs the BMAD test matrix and repository verification without removing or
   weakening any baseline job or step.
2. The gate uses only repository files and declared baseline tools in a clean
   checkout.

**Verification:** `bash scripts/test-verify-bmad.sh && bash scripts/verify-bmad.sh`

**Budget:** 2000 tokens

**Dependencies:** WP-009, WP-010, WP-011, WP-012

### WP-014: Run Reproducibility and Full Regression Verification

**Territory:** Bootstrap and Locking

**Contract Clause:** FR-011, FR-012; NFR-004, NFR-006; AC-012–AC-015

**Files:**
- Modify: `bmad.lock.json` only if final normalized generated identities require refresh
- Modify: `docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md`

**Acceptance Criteria:**

1. Disposable regeneration from the exact lock reproduces generated framework
   and launcher files without changing human-owned context or artifacts.
2. BMAD governance tests and verifier pass.
3. Existing formatting, Clippy, workspace all-target tests, rustdoc, packaging,
   audits, portability, documentation, local proof, and applicable Docker proof
   contracts remain present and pass in proportion to local prerequisites.
4. Product source, current study content, and public mdBook pages are unchanged.

**Verification:** `bash scripts/test-verify-bmad.sh && bash scripts/verify-bmad.sh && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --all-targets --locked`

**Budget:** 5000 tokens

**Dependencies:** WP-003, WP-004, WP-005, WP-006, WP-007, WP-008, WP-013

## Execution Order

1. Independent start: WP-001 and WP-002.
2. Supply-chain discovery: WP-003.
3. Generated installation: WP-004.
4. Parallel governance foundation: WP-005 and WP-006.
5. Parallel implementation after foundations: WP-007, WP-009, and WP-010.
6. Onboarding and remaining guards: WP-008, WP-011, and WP-012.
7. CI integration: WP-013.
8. Final reproducibility and regression sweep: WP-014.

## Baseline Verification

- **Tests:** PASS — `cargo test --workspace --all-targets --locked` exited 0 on
  2026-08-14. The first sandboxed attempt failed only because Wiremock could not
  bind loopback ports; the approved local-port rerun passed all targets.
- **Dependencies:** OK — Node 20.18.0, npm 10.8.2, Python 3.10.0, uv 0.9.25,
  Rust 1.92.0, Git, Codex, and Claude Code are available. BMAD 6.11.0 requires
  Node 20.12 or newer.
- **Package inspection:** OK for planning — exact npm metadata, tarball
  integrity, package hash, license, executable, module definition, CLI options,
  and Codex/Claude target roots were inspected without executing the installer.
  Full dependency and disposable-install inspection is WP-002 and WP-003.
- **Paths:** OK — planned human-owned paths are repository-relative and absent or
  existing as declared; generated patterns are confirmed by the inspected
  installer metadata and will be resolved to an exact inventory in WP-003.
- **Branch:** `codex/heist-bmad-repository-setup`, created from
  `88385f60adb116a04fc3834152c6d40f7b8fb916` (`master`).
- **Isolation:** Branch-only isolation is used. A separate worktree was not
  created because the signed planning artifacts were already uncommitted in the
  shared checkout and all three territories have disjoint write ownership with
  dependency ordering at their boundaries.
- **Conflicts:** None observed; the base checkout was clean before Heist planning,
  and only `docs/gangsta/` is currently untracked.
