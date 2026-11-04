# Don decisions: re-plan and November dependency refresh, 2026-11-04

This decision record is approval evidence for the artifact index. It is not
part of the public mdBook. It contains no key material.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-11-04T21:45:15Z`.
- Channel: interactive Claude Code session. A planning roundtable of agent
  personas (Product Owner, AI CTO, Chief Technologist; the Don was the only
  human), then plan approval, then bmad-build CHECKPOINT 1 for the amended
  dependency spec. The CHECKPOINT answer was received in the session shortly
  before this record was written.
- Prior approval of the same artifact:
  `2026-10-26-don-approval-rustsec-2026-0382.md` (the `rustls-webpki`
  change, kept unchanged).
- Classification: `repository-visible`.
- Independence exception: the Don owns these artifacts and is the only
  registered human authority.

## Re-plan decisions

The Don approved a re-baselined plan through the end of January 2027. On
2026-11-04, S3 was 12 days behind the months 1–2 schedule and the 2026-11-13
month-1 exit was unreachable. The Don's selections were:

1. Capacity: "Bursty / unpredictable". Small increments, each independently
   valuable, with at most two open pull requests.
2. FOSDEM 2027:
   - "Submit main track on S1/S2 (Recommended)", by 2026-11-15;
   - "Main + AI Plumbers (Recommended)": a narrower technical talk to the AI
     Plumbers devroom by 2026-11-27, telling each set of organisers about the
     other submission.
   - The FOSDEM main-track submission becomes the new month-1 exit.
3. Merging: "You review and merge". The agent opens pull requests and stops.
4. Dependabot: "Batch into the RustSec spec (Recommended)".
5. Demo design: "Stage A of pilot-r2, protected first (Recommended)".
   - The Stage A protocol pull request carries the protection marker, the
     lock and the tree identity, and merges before any paid call.
   - Results live outside the protected tree.
   - Stage A runs on the maintainer's arm64 Mac, with the amd64 agent images
     under emulation. Emulation affects timing, timeouts and therefore pass
     rates, but not token or cost accounting. The run is valid for token and
     cost claims only, and this is disclosed.
   - Stage B is optional.
6. CLI pins: "Re-pin in S3a, freeze at smoke (Recommended)".
   - Re-pin to the latest Claude Code and Codex releases after a no-cost
     keyless re-capture.
   - Freeze the pins once the smoke trial passes, until FOSDEM.
   - Keep the S2 fixtures as "before" data.
7. R1 and the S2 key: "R1 + revoke S2 key now (Recommended)".
   - A fresh key with a USD 0.25 limit, a 24-hour expiry and model and
     provider policies; run cap USD 0.05, after the abstract is submitted.
   - S2 findings become citable under the plan's credibility rules: dated,
     versioned, with sample size, no ranking or performance claim.
   - The old S2 key is to be revoked.

Registration: the re-baselined plan will be recorded as an amendment to
`forgetest-roadmap-2026-10` and re-approved in the index once PR #54 merges.
The roadmap's latest version is on that branch, so amending it here would
conflict.

Rescheduled:
- The mid-December exit becomes Stage A results with a billed-versus-stream
  check, by 2026-12-21.
- v0.2 moves to 2027-01-22.
- Lock v2, the S7 egress seal, the toolchain bump and `ci.yml` re-baseline
  items move to February 2027.

## S2 key revocation

The Don answered "Revoked now". A check at 2026-11-04T21:44:53Z still returned
HTTP 200 from `GET /api/v1/key`, so revocation is **not yet confirmed**. The
Don owns the follow-up, due by 2026-11-06. If the key still answers by then,
the urgent security procedure applies: a minimal incident record approved by
the Don. The confirming check (HTTP 401) will be recorded in a separate
addendum record, so this record stays unchanged as approval evidence.

## Approved artifact

The Don approved the amended dependency spec at CHECKPOINT 1 ("[A] Approve
(Recommended)"). The SHA-256 below was computed over the amended spec at
approval. Later workflow edits to the same file (task checkboxes, `status`,
the verification record) are not part of the approved content.

| Artifact ID | Path | SHA-256 at approval |
|---|---|---|
| `spec-rustls-webpki-rustsec-2026-0382` | `_bmad-output/implementation-artifacts/spec-rustls-webpki-rustsec-2026-0382.md` | `17964bf9fcb16d544da5387f5a36a7a5f5865a41f82d4c015a80b34b6aca1499` |
