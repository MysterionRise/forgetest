# Don approval of S1 and S2, 2026-10-18

This decision record is approval evidence for the artifact index. It is not
part of the public mdBook.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-18T22:36:33Z`.
- Channel: interactive Claude Code session, bmad-build CHECKPOINT 1.
- Classification: `repository-visible`.
- Basis: `governance-lean-bmm-ruling` R1 and the months 1-2 roadmap amendment.
- Independence exception: the Don owns these artifacts and is the only
  registered human authority.

## Don statements

- S1 agent-images: "[A] Approve, keep full spec". This accepts about 2,300
  tokens, above the 1,600-token guideline.
- S2 live-spike: "[A] Approve, keep full spec".

## Paid-run approval for S2 (ruling R3)

Approving S2 authorizes one fact-finding spike under these conditions:

- a per-run cap of USD 2;
- a key the Don creates in the OpenRouter dashboard, with a USD 2 limit, an
  expiry of 48 hours or less, and a model allowlist limited to the four study
  models;
- execution in throwaway containers on the maintainer's machine, which serve
  as the disposable environment;
- web tools disabled on every call;
- no subscription credentials;
- billed usage read from `GET /api/v1/key` and recorded in the run record.

Spike outputs are labelled fact-finding and are not published as evidence.

## Approved artifacts

| Artifact ID | Path | SHA-256 at approval |
|---|---|---|
| `spec-agent-images` | `_bmad-output/implementation-artifacts/spec-agent-images.md` | `d10a1bef613a9d53de9f73335f7672aa94683cca3bd78c4ed043c1b799b750d0` |
| `spec-live-spike` | `_bmad-output/implementation-artifacts/spec-live-spike.md` | `da0124b4acb62e2f17c729ed593654d37d6730090f7fadf5827d62a786456f7d` |
