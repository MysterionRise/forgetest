# Deferred work

- source_spec: `_bmad-output/implementation-artifacts/spec-bmad-repository-setup.md`
  summary: Upgrade the locked h2 dependency to resolve RUSTSEC-2026-0258 under a separately approved dependency-change plan.
  evidence: The unchanged baseline dependency graph contains h2 0.4.13; cargo audit reports the advisory published 2026-08-17 and remediation requires h2 >=0.4.16.
  status: resolved 2026-10-15 by `spec-h2-rustsec-2026-0258` (h2 0.4.20, lockfile-only; PR #52).
