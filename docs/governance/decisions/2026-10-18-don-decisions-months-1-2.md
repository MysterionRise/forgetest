# Don decisions for months 1–2, 2026-10-18

This decision record is outside the public mdBook. It records the Don's answers
from the months 1–2 planning roundtable (Product Owner, AI CTO, Chief
Technologist) and amends `forgetest-roadmap-2026-10`.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-18T20:54:44Z`.
- Channel: interactive Claude Code session. The Don answered planning questions,
  then approved the resulting plan.

## Don statements and interpretations

1. **Endpoint.**
   - Selected: "OpenRouter, guardrailed".
   - Interpretation: published studies run through OpenRouter as a
     pre-registered, disclosed endpoint. Each arm uses an expiring key with
     provider and model allowlists and a USD budget. The roadmap's
     third-party-gateway exclusion is narrowed to forgetest's own code and
     supply chain.
2. **Keys.**
   - Statement: "I WOULD GIVE REASONABLE KEYS IF NEEDED". The Don did not select
     the local-spike exception, the management key, or the Broadcast bucket.
   - Interpretation: no management key and no Broadcast. The Don creates
     reasonably scoped keys when an approved plan needs them, and billed usage
     is read per key.
   - The fact-finding spike will run under its own approved plan (S2) in a
     throwaway container, with a key limited to USD 2 that expires within 48
     hours. That plan must be approved before any key is requested.
3. **Codex sandbox.**
   - Selected: "Try workspace-write first".
   - Interpretation: `danger-full-access` is not approved. If the spike shows
     `workspace-write` cannot work in the capability-dropped container, a new
     Don decision is required.
   - The Claude image follows the existing policy in `docker/agents/README.md`:
     vendor images are not redistributed.
4. **Month 2.**
   - Selected: "Approve re-scope".
   - Interpretation: the meter is cancelled, a minimal egress seal is built in
     month 3 before v1-r2, and Homebrew and `dist` are dropped. The month-2 exit
     is v0.2 on crates.io plus billed-vs-stream reconciliation.
5. **Plan.** The Don approved the months 1–2 plan: specs S1–S7, the schedule,
   the budget of at most USD 20, and the exit criteria. That plan is summarized
   in the roadmap amendment.

## Approved artifact

| Artifact ID | Path | SHA-256 at approval |
|---|---|---|
| `forgetest-roadmap-2026-10` | `_bmad-output/planning-artifacts/forgetest-roadmap-2026-10.md` | `0ad9797e96b8685f3fc7f29d5dec438e5eb00fde28a3e2ef53f7cb01dfa3745b` |
