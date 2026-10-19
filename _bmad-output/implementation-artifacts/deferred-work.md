# Deferred work

- source_spec: `_bmad-output/implementation-artifacts/spec-bmad-repository-setup.md`
  summary: Upgrade the locked h2 dependency to resolve RUSTSEC-2026-0258 under a separately approved dependency-change plan.
  evidence: The unchanged baseline dependency graph contains h2 0.4.13; cargo audit reports the advisory published 2026-08-17 and remediation requires h2 >=0.4.16.
  status: resolved 2026-10-15 by `spec-h2-rustsec-2026-0258` (h2 0.4.20, lockfile-only; PR #52).

- source_spec: none
  summary: Disable web tools in the built-in Codex and Claude Code profiles (Codex `-c web_search="disabled"`, Claude `--disallowedTools WebSearch,WebFetch`), and record the resulting profile-digest change.
  evidence: Split on 2026-10-15 from the month-1 measurement initiative of forgetest-roadmap-2026-10. It ships independently and changes profile digests, which invalidates existing benchmark locks. Due before pilot-r2.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Capture live Claude Code and Codex golden stream fixtures to confirm the usage mapping, then pin it in tests.
  evidence: The mapping is an assumption from vendor docs. It assumes Codex turn.completed usage is cumulative, cached input is part of input_tokens, and reasoning tokens are part of output_tokens. The spec's Design Notes require confirming it before pilot-r2.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Compare the model reported in the agent stream with the requested model before pricing it, and flag any mismatch.
  evidence: Pricing looks up the requested model ID. A routed or aliased model would be priced silently at the requested model's rates. Overlaps with the month-3 integrity flags.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Extend the pricing schema with a provenance field (source URL or note) and tiered, long-context and cache-TTL prices.
  evidence: The flat per-field schema rejects unknown fields such as a source note, so auditors cannot see where prices came from. It also cannot represent tiered vendor pricing.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Record the pricing identity in the benchmark lock and give a specific diagnostic when it mismatches.
  evidence: Pricing is folded only into the policy digest. A lost or edited pricing file surfaces as a generic policy-digest mismatch, and the lock cannot reconstruct the run. Changing the lock schema is an Ask First decision.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Give breaker-truncated runs a distinct CLI exit status, or a machine-readable truncation marker.
  evidence: When the run cost breaker trips, the run still exits 0 and the only signal is a stderr summary, so automation cannot tell truncated evidence from complete evidence.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Mark costs in legacy reports (those without cost_source) as unverified in the HTML and in comparisons.
  evidence: Pre-change reports recorded Claude cost as 0.00 and Codex input including cached tokens. They still render as trustworthy, and comparisons mix old and new token semantics without a warning.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Decide on forward compatibility for new serialized enum values (BudgetUnverifiable, CostSource) under report schema v2: a schema-version policy or a serde(other) fallback.
  evidence: Older v2 readers cannot deserialize reports that contain the new values, even though the schema version stays 2. Bumping the schema is an Ask First decision; candidate for report v3.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Persist whether tokens were reported, so that "0 tokens" and "tokens unknown" can be told apart in reports.
  evidence: tokens_reported exists only during parsing. A stream with a vendor cost but no usage object stores input and output tokens as 0, the same as a genuinely empty run.

- source_spec: `_bmad-output/implementation-artifacts/spec-measurement-stage-0.md`
  summary: Decide how cancelled trials count in headline metrics (rate, Wilson interval, pass@1, pass^3, paired bootstrap) for the report-v3 verdict engine.
  evidence: Breaker-cancelled trials stay in scheduled as non-passes, so truncation lowers observed rates. Interleaved dispatch reduces the skew between agents but does not remove the bias.

- source_spec: `_bmad-output/implementation-artifacts/spec-agent-images.md`
  summary: Record the lock's cli_version from the CLI's stdout only, not the preflight's combined output with container stderr (assigned to S3 by the Don, 2026-10-19).
  evidence: The credential-free lock made on the arm64 Mac records Docker's platform-mismatch warning and Codex's PATH-alias warning in cli_version, so the same images give a different cli_version on the amd64 runner and agents doctor fails across machines.

- source_spec: `_bmad-output/implementation-artifacts/spec-agent-images.md`
  summary: Wire the agent image recipes, the gated agent_images test and scripts/test-build-agent-images.sh into CI (candidate for the S6 governance re-baseline, which edits ci.yml).
  evidence: No CI job builds the recipes or sets FORGETEST_AGENT_IMAGE_TEST, so the gated tests report ok without running, and a stale pin or broken recipe surfaces only at the next manual build-and-lock.

- source_spec: `_bmad-output/implementation-artifacts/spec-agent-images.md`
  summary: Decide where Codex keeps CODEX_HOME in agent containers. Under the agent profile HOME=/tmp/home, Codex refuses to create its PATH helper aliases under /tmp, and the /tmp tmpfs is noexec. S2 measures the effect; S3 may add a dedicated writable home outside /tmp.
  evidence: Under the agent runtime flags Codex 0.163.0 warns "Refusing to create helper binaries under temporary dir /tmp", and the packaged rg was not on PATH in the container. Both come from the existing docker_arguments (HOME=/tmp/home, tmpfs /tmp without exec), not from the image recipes.
