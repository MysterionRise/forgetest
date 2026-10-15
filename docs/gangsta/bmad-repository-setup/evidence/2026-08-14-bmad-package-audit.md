---
heist: bmad-repository-setup
work-packages: [WP-002, WP-003, WP-014]
date: 2026-08-14
last-verified: 2026-08-29
status: blocked
---

# BMAD Package Provenance Audit

## Decision

`bmad-method` 6.11.0 is accepted for the locked, BMM-only bootstrap. Retrieval,
content inspection, integrity verification, production dependency resolution,
and two independent installer executions were performed in disposable locations
before repository installation. Both clean baseline clones produced the same
sanitized shared inventory after the narrow, reviewed wall-clock normalization
documented below. The working repository has not been modified by the installer.

## Registry and Artifact Identity

| Field | Audited value |
| --- | --- |
| Registry package | `bmad-method@6.11.0` |
| Tarball | `https://registry.npmjs.org/bmad-method/-/bmad-method-6.11.0.tgz` |
| Registry SRI | `sha512-xP+ZFPa8eA/xa9O9LSW7u8xq3/cVqEaCfdRHYwqti/JFm9+F8/Zmp+UwhHUMPAoBhyOCbDtwOJedCEnFTqqz0Q==` |
| Tarball SHA-512 (hex) | `c4ff9914f6bc780ff16bd3bd2d25bbbbcc6adff715a846827dd447630aad8bf2459bdf85f3f666a7e53084750c3c0a018723826c3b7038979d0849c54eaab3d1` |
| Registry SHA-1 shasum | `9e61bc08eacadb2f2323f9d08f756950406b1823` |
| Registry signing key | `SHA256:DhQ8wR5APBvFHLF/+Tc+AYvPOdTpcIDqOhxsBHRwC7U` |
| Registry attestation | `https://registry.npmjs.org/-/npm/v1/attestations/bmad-method@6.11.0` |
| Package content | 344 files; 3,098,946 unpacked bytes |
| License | MIT |
| Main and both bins | `tools/installer/bmad-cli.js` |
| Declared Node engine | `>=20.12.0` |
| Baseline Git revision | `88385f60adb116a04fc3834152c6d40f7b8fb916` |

The independently calculated binary SHA-512 encoded as base64 exactly matched
the registry SRI payload. The independently calculated SHA-1 also matched the
registry shasum. Tar listing and manifest extraction were read-only.

## Lifecycle Review

| Lifecycle surface | Finding |
| --- | --- |
| `preinstall` | Not declared |
| `install` | Not declared |
| `postinstall` | Not declared |
| `prepare` | `command -v husky >/dev/null 2>&1 && husky || exit 0` |
| Audit resolution | `npm install --ignore-scripts --omit=dev --no-audit --no-fund --package-lock=true` |
| Resolved production packages with install scripts | 0 |
| Installer execution | Two direct, offline executions in clean disposable clones |

Lifecycle execution remained disabled during dependency resolution. The already
inspected package CLI entry point was then invoked directly for the two
disposable generation exercises; no mutable package selector or lifecycle hook
selected executable code.

## Dependency Resolution

Resolution used a freshly extracted package with Node 20.18.0 and npm 10.8.2.
`npm ls --omit=dev --all --json` reported 13 direct dependencies, 98 resolved
production package nodes, and zero problems. The immutable, path-sensitive
production lock payload is committed at `bmad.lock.json#/dependencies/nodes`;
its canonical SHA-512 is
`b0be3143c73bee6774c92759a9d5d69eed0a44bf0ace7f36bf57c0f745ea8fb0217ddcd2f0fc52f98ab91915b141c7032aece1121754f1087afefee643bb1e99`.

| Dependency | Declared | Resolved |
| --- | --- | --- |
| `@clack/core` | `^1.3.1` | `1.4.3` |
| `@clack/prompts` | `^1.4.0` | `1.7.0` |
| `@kayvan/markdown-tree-parser` | `^1.6.1` | `1.6.1` |
| `chalk` | `^4.1.2` | `4.1.2` |
| `commander` | `^14.0.0` | `14.0.3` |
| `csv-parse` | `^6.1.0` | `6.2.1` |
| `glob` | `^11.0.3` | `11.1.0` |
| `ignore` | `^7.0.5` | `7.0.6` |
| `js-yaml` | `^4.1.0` | `4.3.1` |
| `picocolors` | `^1.1.1` | `1.1.1` |
| `semver` | `^7.6.3` | `7.8.5` |
| `xml2js` | `^0.6.2` | `0.6.2` |
| `yaml` | `^2.7.0` | `2.9.0` |

Each transitive node in the lock includes its npm path, exact version, registry
integrity, declared edges, and optional edges. This preserves duplicate nested
versions and makes the executable dependency graph independently checkable.

## Runtime, Platform, and Generation State

| Field | Audited value |
| --- | --- |
| Node | 20.18.0 |
| npm | 10.8.2 |
| OS | Darwin / macOS 15.7.4 |
| Architecture | arm64 |
| Audit location | Fresh disposable directory outside the repository |
| Lifecycle mode | Disabled |
| Repository mutation by installer | None |
| Generation environment | Same exact runtime and platform, two clean disposable clones |
| Generated-path/file inventory | Verified and recorded in `bmad.lock.json` |

An initial network-enabled resolution inherited Node 16/npm 8 from the elevated
shell. That result was rejected. Resolution was repeated from a fresh disposable
directory with the exact locked runtime; only the latter result appears in the
lock and this evidence.

## Baseline CI Inventory

The lock records the baseline workflow at
`88385f60adb116a04fc3834152c6d40f7b8fb916:.github/workflows/ci.yml`, its raw
SHA-256, five ordered jobs, and 34 ordered steps. Job and step semantic hashes
use recursively key-sorted, compact UTF-8 JSON. Job hashes exclude the step
array; step hashes include every field. This allows additive CI work while
retaining independently comparable baseline semantics.

The current additive workflow has SHA-256
`3ec5cb6452290cd608adc2e76d9984b26e73557776a7f9a2b9002de4433ba729`.
Its marker-delimited governance job provisions exact Python `3.10.0` with
`actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97` (`v7.0.0`)
and rebuilds a clean `target/pages` before invoking the sole public verifier
entrypoint. The canonical job is validated byte-for-byte, including the fresh
render and both normal verifier commands.

## Upgrade Policy and Scope

The accepted optional scope is only `bmm`; the enabled tools are `codex` and
`claude-code`; the output root is `_bmad-output`. Version 6.11.0 remains
exactly locked. Only a reviewed 6.11.x patch is eligible under this setup.
Automatic updates are prohibited, and every minor or major version requires a
new approved plan.

## Disposable Installation Exercise

The exact inspected CLI entry point was executed directly from the locked
package and lifecycle-disabled production dependency tree. No `npx`, dist-tag,
registry selector, or mutable package lookup selected executable code. The npm
update lookup was forced offline; no external module fetch occurred.

The exact command is below with only the exact Node binary location and
disposable clone location normalized:

```text
env -u GITHUB_TOKEN -u GH_TOKEN -u NPM_TOKEN \
  npm_config_offline=true npm_config_update_notifier=false NO_UPDATE_NOTIFIER=1 \
  node@20.18.0 tools/installer/bmad-cli.js install \
  --directory <disposable-clone> --modules bmm \
  --tools codex,claude-code --user-name Contributor \
  --communication-language English --document-output-language English \
  --output-folder _bmad-output --set core.project_name=forgetest \
  --set bmm.user_skill_level=expert \
  --set bmm.planning_artifacts=_bmad-output/planning-artifacts \
  --set bmm.implementation_artifacts=_bmad-output/implementation-artifacts \
  --set bmm.project_knowledge=_bmad-output/project-knowledge --yes
```

Each clone was clean and detached at
`88385f60adb116a04fc3834152c6d40f7b8fb916` before execution. Both invocations
exited successfully. Normalized output common to the two runs was:

```text
exit_code=0
2 module(s) installed
BMad Core Module (v6.11.0, installed)
BMad Method (v6.11.0, installed)
codex configured: 49 skills -> .agents/skills
claude-code configured: 49 skills -> .claude/skills
Created directories: output folder: _bmad-output
```

## Raw Installer Inventory

| Check | Result |
| --- | --- |
| New top-level roots | `.agents`, `.claude`, `_bmad`, `_bmad-output` only |
| Framework files | 21 under `_bmad` |
| Codex launchers | 248 files / 49 skills under `.agents/skills` |
| Claude launchers | 248 files / 49 skills under `.claude/skills` |
| Launcher parity | Byte-identical relative inventories |
| Generated inventory | 741 paths: 224 directories and 517 files |
| Empty directories | `_bmad-output` only |
| Tracked baseline modifications | 0 |
| Symlinks / special files | 0 / 0 |
| Absolute / traversal paths | 0 / 0 |
| Duplicate portable targets | 0 |
| Human-owned output files | 0 |
| Unexpected roots, modules, tools | 0 |
| Packaged bytecode caches | 10 `.pyc` files in 8 `__pycache__` directories |
| Installer-created personal state | `_bmad/config.user.toml` and `_bmad/custom/config.user.toml` |

The bytecode caches embedded an upstream runner home path and are unsuitable
for shared launcher promotion. The header of `_bmad/config.user.toml` describes
its contents as personal installer answers; `_bmad/custom/config.user.toml`
describes itself as personal state and is ignored by
`_bmad/custom/.gitignore`. Neither file is part of the shared tree.

## Sanitized Promotion Inventory

Both promotion trees remove exactly those 10 bytecode files, their eight
now-empty cache directories, and both personal configuration files. Neutral
repository-wide values are instead pinned in the upstream-designated committed
team override, `_bmad/custom/config.toml`: `core.user_name=Contributor`,
`core.communication_language=English`, and
`modules.bmm.user_skill_level=expert`. That customized file is human-owned, its
identity is recorded separately, and it is excluded from generated paths and
generated-file hashes.

| Check | Promoted result |
| --- | --- |
| Sanitized framework files | 19 under `_bmad`: 18 generated and 1 human-owned |
| Codex launchers | 243 files / 49 skills under `.agents/skills` |
| Claude launchers | 243 files / 49 skills under `.claude/skills` |
| Sanitized filesystem | 721 paths: 216 directories and 505 files |
| Shared generated inventory | 715 paths: 211 directories and 504 hashed files |
| Excluded namespace containers | `.agents`, `.agents/skills`, `.claude`, `.claude/skills` |
| Human-owned team override | `_bmad/custom/config.toml`, SHA-256 `9fe1c15a777df0634d8748ed24c83f75667e27bfe692c0b6368633018606bd04` |
| Human-owned output root | `_bmad-output`, empty after installation |
| Launcher parity | Byte-identical relative inventories |
| Machine-local path scan | No `/home/`, `/Users/`, `/private/`, or Windows user paths |
| Credentials / symlinks / special files | 0 / 0 / 0 |
| First raw generated-tree SHA-256 | `785bce643392e6e536e27ec34acf32ca6a056b0488365ccfda9ef5101c7ca8b6` |
| Second raw generated-tree SHA-256 | `d72aeb2cce2e224fb8d8166a558ed401affca9af8fdc18e9a61edcd370755d61` |
| Normalized reproducibility SHA-256 | `3ed38cac3ffb634ae8f46bbc62ce279f8d6d5c562f4d9818790438cd0601b792` |
| Generated path-set SHA-256 | `aae6e0dc0c0b1932d46d46fd8822b46589f45a0064b87dc2feb60fe9d0e82203` |
| Launcher tree SHA-256 | `0164e21e498599c619505ea64e76ca2f81ca4a4b1ac3abdad94af9a99c0a48cb` |

Every shared generated file has its first promotion's raw-byte SHA-256 in
`bmad.lock.json#/generation/inventory/generated_files_sha256`. The canonical
digest covers sorted repository-relative generated paths, hashes, and the empty
symlink list. Namespace containers, the human-owned output root, and the
human-owned team override are not claimed as generated. No local clone/cache
path, account username, credential, or authorization material is stored in
committed fields.

The manifest contains only bundled `core` and `bmm`, with tools `codex` and
`claude-code`. Configuration records `Contributor`, English communication and
output, expert skill level, and the three approved `_bmad-output` paths.

## Independent Reproducibility Comparison

The two sanitized filesystem path and type sets matched exactly. Raw bytes
differed in exactly four files:

- `_bmad/_config/manifest.yaml`
- `_bmad/core/config.yaml`
- `_bmad/bmm/config.yaml`
- `_bmad/_config/files-manifest.csv`

The first three contain eight installer wall-clock values: six manifest
`installDate`/`lastUpdated` values and one `# Date:` header in each generated
module configuration. The fourth contains three SHA-256 cells that directly
hash those three timestamp-bearing files. Review therefore authorized a narrow
eleven-value comparison normalization: replace only those eight time values and
recompute only the three matching manifest hash cells. No path, other field, or
other byte is normalized. After that transformation all 504 generated file
identities matched and both trees produced
`3ed38cac3ffb634ae8f46bbc62ce279f8d6d5c562f4d9818790438cd0601b792`.
The human-owned team override was also byte-identical between runs.

Both inventories contained zero unsafe or absolute paths, traversal segments,
symlinks, special files, duplicate Unicode-NFC/case-folded targets, output-root
overlap, machine-local paths, credential-pattern hits, unexpected roots, extra
modules, or extra tools. The first sanitized tree remains the approved source
for the repository installation step; its machine-local location is intentionally
not committed.

## Initial Disposable Inventory Test State

At the initial 2026-08-14 audit boundary,
`bash scripts/test-verify-bmad.sh --case disposable-inventory` failed with
`ERROR: missing production verifier: scripts/verify-bmad.sh`. That data package
could not make the then-future production test green. Focused lock, hash, scope,
path, and inventory assertions provided the GREEN evidence for the data
transition; the 2026-08-29 recheck below records the completed verifier result.

Repository installation must copy the first sanitized tree; raw installer
output is evidence only and must never be promoted directly.

## Review Notes

- Repository installation must add an anchored `/_bmad/config.user.toml` rule
  to the root `.gitignore` while retaining the nested custom `*.user.toml`
  ignore. Root `.gitignore` is deliberately unchanged by this audit.
- Reproducibility permits only the eleven reviewed timestamp and directly
  derived manifest-hash values. Any other byte drift blocks promotion.
- npm emitted the upstream deprecation warning for resolved direct dependency
  `glob@11.1.0`; the identity is frozen and visible in the lock for review.
- Development-only packages were omitted from the executable graph.
- No package cache location, temporary path, home path, credential, token, or
  machine-specific account identifier is recorded.

## 2026-08-29 Reproducibility and Regression Recheck

The exact cached `bmad-method@6.11.0` tarball was checked again against the
locked SHA-512, and all 98 production dependency archives were reconstructed
from their locked integrity identities. The inspected installer entry point was
run directly in a disposable clean clone at the locked baseline with Node
20.18.0 and npm 10.8.2. After removing only the already-reviewed bytecode and
personal-state files and applying the eleven-value comparison normalization,
the result matched all 715 generated paths and all 504 normalized generated-file
identities. A separate disposable exercise retained all seven human-owned files
byte-for-byte. The installer was not run in the working checkout.

The completed regression results were:

Before the production verifier was mechanically split, the complete harness
transcript and real-checkout transcript were captured. After the split, their
SHA-256 values remained byte-identical at
`05e1a7eb607b4c0c9818a6e42745d6bcf8b85f2ddb5a3c662a69db0c0c31733f`
and `c4ed06e64909ff5154e7b850f714fcd5f783018a8dbeb38dc84c290273dff66b`,
respectively. The Python 3.10+ standard-library implementation now consists of
one coordinator and twelve focused modules; the largest production verifier
Python file is 454 physical lines, the largest test implementation is 465
physical lines, and the production schema remains singular. Every verifier and
test implementation file is below 500 physical lines.

The Don-approved specification review is recorded in project context at
`2026-08-29T12:15:33Z` under the existing repository-local alias model. It does
not establish a new external or cryptographic identity binding. Onboarding now
separates the renderer's `uv` plus PEP-723 Python `>=3.11` runtime from the
repository verifier's Python `>=3.10` runtime.

| Gate | Result |
| --- | --- |
| BMAD fixture harness and production verifier | PASS: 211 assertions covering fixture isolation, deterministic/runtime read-only behavior including all 13 missing production helpers, 32 named negative fixtures, and focused adversarial guards |
| Additive CI baseline reconstruction | PASS: current workflow SHA-256 `3ec5cb6452290cd608adc2e76d9984b26e73557776a7f9a2b9002de4433ba729`; stripping the complete in-quality marker block recovered SHA-256 `32f4bbe521b200cae48cb32821713318a8d9d04d37434a166e1018fcc4bd63f0` |
| CI rendered-publication placement | PASS: the exact gate begins at baseline byte offset 1,398 immediately after `bash scripts/test-build-pages.sh`; the persisted `target/pages` artifact passed required rendered-output scanning |
| Bash, Python, lint, and size gates | PASS: Bash 3.2 syntax, Python 3.10.0 compilation, Ruff, diff whitespace, and the per-file `<500` limit |
| Launcher parity | PASS: 243 files in each launcher root and 49 mapped `SKILL.md` runtime entrypoints |
| Local `bmad-build` render smoke | PASS on the host-access rerun: `bash scripts/test-bmad-render.sh` printed `BMAD bmad-build launcher render smoke passed`; the verified immutable generation is `_bmad/render/bmad-build/forgetest-b268a235b923/027ceb3b49fb1d543e81/workflow.md` |
| Rust formatting, all-target Clippy, workspace all-target tests, and rustdoc | PASS |
| Release portability, mdBook Pages, publisher, and sample-report checks | PASS |
| Workspace package dry-run | PASS; upstream `chacha20` 0.10.1 yanked warning retained |
| License and source policy audit | PASS |
| Twelve-task calibrated suite validation | PASS |
| Local snippet and repository demos | PASS |
| Installed-binary version and suite smoke checks | PASS |
| ARM64 Linux release cross-build | PASS; output verified as an AArch64 ELF executable |
| Pinned Rust runner image build | PASS |
| Docker-enabled runner and agent tests | PASS: 56 tests |
| Docker snippet and repository demos | PASS |
| Product, protected-study, and public-mdBook change boundary | PASS: no changed paths |
| RustSec audit | FAIL: one newly published advisory described below |

The first sandboxed workspace-test attempt could not bind Wiremock loopback
ports. The permitted local-port rerun passed every workspace target. On the
Apple Silicon verification host, `cross` 0.2.5's pinned builder image was
available only for `linux/amd64`; running that same pinned image under Docker's
amd64 emulation completed the required `aarch64-unknown-linux-gnu` release
build. These were host adaptations only and did not change repository files or
gate semantics.

`cargo audit` loaded 1,226 advisories and reported one vulnerability in the
locked dependency graph:

| Field | Finding |
| --- | --- |
| Advisory | `RUSTSEC-2026-0258` |
| Crate | `h2` 0.4.13 |
| Title | `h2 unbounded empty DATA frames` |
| Published | 2026-08-17 |
| Fixed versions | 0.4.16 or newer |
| Additional allowed warning | `chacha20` 0.10.1 is yanked |

The advisory was published after the 2026-08-14 baseline and requires a locked
dependency change. Dependency changes are outside this approved implementation
scope and require a new approved plan, so this recheck records the failure
without changing `Cargo.lock` or weakening the audit gate. All other applicable
proof gates passed.
