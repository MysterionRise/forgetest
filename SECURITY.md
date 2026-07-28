# Security Model

## Scope

`forgetest` executes code produced by coding agents. Its controls reduce
accidental exposure and constrain audited evaluation fixtures. They do not make
arbitrary hostile repositories or generated native code safe.

The trusted computing base includes the host kernel, Docker daemon and runtime,
Rust toolchain, verifier image, `forgetest` binary, suite metadata, hidden
grader, and CI runner.

## Trust Levels

| Mode | Intended use | Network | Isolation statement |
|---|---|---|---|
| Local snippet | Trusted development | Host policy | Temp project and cleared child environment; not a sandbox |
| Docker snippet | Bundled allowlisted cases | None | Hardened container boundary |
| Local repository | Audited suites during development | Agent CLI may use network; grader uses host | Process isolation only; not for publication |
| Benchmark repository | Published audited studies | Agent bridge; verifier none | Ephemeral agent container plus independent hardened verifier |

## Repository Benchmark Boundary

### Agent container

- Fresh per-trial visible workspace.
- Read-only root filesystem.
- Non-root numeric user.
- `cap-drop=ALL` and `no-new-privileges`.
- Memory, CPU, PID, tmpfs, timeout, and output limits.
- Explicit credential environment allowlist.
- Isolated home; host home and Docker socket are not mounted.
- Network is available only because hosted agent APIs require it.
- Built-in profiles grant noninteractive edit and command execution inside this
  outer container; the outer container, not an agent permission prompt, is the
  security boundary.

The agent container is not trusted to grade itself. Its Git state is ignored.

### Trusted patch capture

The engine snapshots the visible workspace before and after agent execution.
It computes changed files and patch content itself, with file-count,
workspace-size, and patch-size limits. A retry receives a restored pristine
workspace.

### Verifier container

- Fresh clean workspace reconstructed from the original plus trusted change
  set.
- Hidden grader overlaid only after agent termination.
- `--network none`.
- Read-only root filesystem.
- Non-root numeric user.
- `cap-drop=ALL` and `no-new-privileges`.
- Per-trial tmpfs target directory; no shared build cache.
- Memory, CPU, PID, output, and total grader-time limits.
- Unique container names and forced removal on timeout/error.
- Pinned Rust release and locked offline dependency cache.

Only the verification workspace is mounted. Secrets, host home, Docker socket,
agent caches, and other trial workspaces are not mounted.

## Configuration Trust Boundary

Precedence is CLI flags, explicit config file, then user/default config.

Repository suite files may define task content, prompt, grader commands,
timeouts, and provenance. They cannot select provider hosts, credentials,
agent/verifier images, Docker network mode, host environment interpolation, or
trusted benchmark policy. Unknown fields are rejected.

Credential interpolation is limited to documented API-key variables. Child
processes receive an explicit allowlist after `env_clear`.

## Budgets and Cleanup

- Agent wall time, output, reported token/cost, and retries are explicit policy
  inputs.
- Normalized agent traces have a fixed 10,000-event safety ceiling in addition
  to the configured raw output-byte limit.
- Verifier wall time and output are bounded.
- Unix children run in a new process group; timeout/output termination kills
  the process tree and reaps the child.
- Docker containers have unique names and are force-removed after abnormal
  termination.
- Every scheduled trial is persisted, including infrastructure failures.

Token and cost enforcement depends on usage fields reported by an agent vendor.
It is not an independent billing meter.

## Evidence and Redaction

Private raw bundles may contain prompts, source code, model messages, tool
events, paths, diagnostics, and credential-shaped accidental output. Treat them
as sensitive.

`forgetest redact`:

- Replaces configured workspace/home paths.
- Replaces known secret values and common API key/token patterns.
- Removes private-reasoning-shaped object keys.
- Removes raw vendor events and free-form model messages.
- Replaces retained event messages with fixed category labels.
- Removes private artifact references.
- Marks the report with redaction version, time, and replacement count.

The public and private bundles each receive a deterministic SHA-256 artifact
inventory. Checksums detect file changes; they are not a signature or
attestation.

Review public output before publication. Pattern-based redaction cannot
guarantee removal of every possible secret encoding.

## Supply Chain

- Rust is pinned in `rust-toolchain.toml`.
- Cargo operations use committed lockfiles in CI and the verifier image.
- The verifier resolves dependencies offline.
- Snippet Docker tasks accept only the bundled dependency allowlist.
- Benchmark locks require full immutable OCI image digests.
- CI runs RustSec, license/source, package, documentation, test, and lint gates.
- Release automation produces checksums, an SBOM, and build provenance
  attestations; those claims apply only to completed release workflows.

## OWASP LLM Risk Mapping

| Risk area | Control |
|---|---|
| Prompt injection / excessive agency | Agent is isolated, bounded, and cannot choose the trusted grader or policy |
| Insecure output handling | Model output becomes an untrusted patch verified independently |
| Sensitive information disclosure | Explicit environment allowlists, isolated homes, private raw bundle, public redaction |
| Supply chain | Locked toolchains/dependencies, immutable benchmark images, provenance records |
| Unbounded consumption | Time, output, memory, CPU, PID, retry, token, and cost budgets |

## Non-Guarantees

v1 does not defend against:

- Kernel, Docker daemon/runtime, Rust compiler, linker, or dependency exploits.
- Denial of service within or below configured host/container limits.
- Covert exfiltration through an agent's required API network.
- Malicious tasks approved by a trusted suite author.
- Secrets encoded in forms not recognized by redaction.
- Host compromise when the local runner or local repository grader is used.

Do not run unreviewed third-party suites. Do not mount the Docker socket into an
agent image. Use disposable, patched CI workers for published studies.

## Reporting Vulnerabilities

Use a private GitHub security advisory for host escape, secret disclosure,
policy bypass, hidden-grader exposure, or evidence-integrity issues. Include the
affected commit, runner mode, operating system, Docker version, and a minimal
reproduction without live credentials.
