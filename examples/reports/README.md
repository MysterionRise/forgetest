# Demo Reports

These artifacts were generated on 2026-07-27 from the working tree represented
by each report's provenance fields. `git_dirty = true` is intentional and makes
clear that these are implementation samples, not signed release evidence.

| Directory | Path exercised | Evidence type |
|---|---|---|
| `snippet-local/` | Mock provider plus local snippet runner | JSON, HTML, SARIF |
| `docker/` | Mock provider plus Docker snippet runner | JSON, HTML, SARIF |
| `repository-local/` | Scripted agent plus local verifier | Redacted v2 bundle |
| `repository-docker/` | Scripted agent plus Docker verifier | Redacted v2 bundle |

Generation commands:

```bash
cargo run --locked --bin forgetest -- \
  demo --runner local --output /tmp/forgetest-snippet-local --format all

cargo run --locked --bin forgetest -- \
  demo --runner docker --output /tmp/forgetest-snippet-docker --format all

cargo run --locked --bin forgetest -- \
  demo --mode repository --runner local \
  --output /tmp/forgetest-repository-local --format all

cargo run --locked --bin forgetest -- \
  demo --mode repository --runner docker \
  --output /tmp/forgetest-repository-docker --format all
```

Each output path must be absent or empty before generation. The commands above
are provenance examples; committed artifact replacement is a deliberate review
step.

Only each repository demo's redacted `public/` directory is copied here. The
snippet reports use the deterministic `MockProvider`; repository reports use
the deterministic scripted agent. Compilation, test execution, grading,
reporting, and Docker isolation are real. No artifact in this directory is a
paid/API model result or a comparative benchmark claim.

CI runs `bash scripts/verify-sample-reports.sh` to reject stale schemas,
retired latency fields, failed demo outcomes, incorrect runner provenance, or
artifact size/hash mismatches.
