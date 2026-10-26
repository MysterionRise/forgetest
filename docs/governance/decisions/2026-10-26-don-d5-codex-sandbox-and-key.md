# Don decisions at the S2 D5 go/no-go, 2026-10-26

This decision record is approval evidence for the artifact index. It is not
part of the public mdBook. It contains no key material.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-26T15:15:03Z`.
- Channel: interactive Claude Code session, D5 go/no-go after the S2 live
  spike (`results/spike-2026-10/FINDINGS.md`).
- Classification: `repository-visible`.
- Related records: `2026-10-18-don-approval-agent-images-and-live-spike.md`
  (the original S2 approval and its R3 paid-run conditions),
  `2026-10-18-don-decisions-months-1-2.md` (decision 3, "Try workspace-write
  first"; a failure requires a new Don decision) and
  `2026-10-22-don-s2-key.md` (key conditions, including revocation after the
  run).
- Independence exception: the Don owns these artifacts and is the only
  registered human authority.

## Facts put to the Don

1. Under the agent container flags, Codex 0.163.0 with
   `--sandbox workspace-write` could not run any command or write any file
   in the live canary: bubblewrap cannot create a user namespace under
   Docker's default seccomp profile. Claude Code with `bypassPermissions`
   fixed the same canary under the same flags.
2. The spike spent USD 0.005034875. The key, which is in the session
   transcript, stays valid until 2026-11-21 unless revoked.

## Don statements and interpretations

1. Codex mode: selected "danger-full-access in container (Recommended)",
   described as "The hardened container becomes Codex's only sandbox:
   read-only root, all capabilities dropped, non-root user,
   no-new-privileges, resource limits. That's the same setup Claude already
   gets with bypassPermissions, so both arms are symmetric. S3 sets it, and
   it's pre-registered and disclosed." Not chosen: "Relax container seccomp",
   "Keep workspace-write, report it".
   - S3 changes the built-in Codex profile from `workspace-write` to
     `danger-full-access`. This is the new Don decision that the months 1–2
     record requires; it does not loosen the container flags.
   - pilot-r2 pre-registers the mode and discloses it in the methods note.
2. Key: "Keep using it for now, no issues", in answer to the request to
   revoke the key. This replaces the 2026-10-22 requirement to revoke the key
   immediately after the run. The key stays active at the Don's choice, and
   the S2 run record says so. It is not used for S5 published runs, which
   need per-arm keys with model and provider allowlists.
   - Deviation acknowledged: the run ended on 2026-10-22 at 01:30 UTC, and
     the key was not revoked as the 2026-10-22 record required. It remained
     active until this decision.
   - Scope that still applies: any paid run with this key is a new paid run
     under ruling R3, needing its own approved plan and cap. The key ends at
     its expiry on 2026-11-21 or when the Don revokes it.
   - Exposure: the key is in the local Claude Code session transcript. The
     Don, who owns the key and is the registered security authority,
     accepted that exposure; no incident record is opened.

## Approved artifact

The SHA-256 below was computed over the spec at the time of these decisions.
Later workflow edits to the same file (task checkboxes, `status`) are not part
of the approved content.

| Artifact ID | Path | SHA-256 at approval |
|---|---|---|
| `spec-live-spike` | `_bmad-output/implementation-artifacts/spec-live-spike.md` | `483bcf1d345c4a09a20d3b118b352e7dc9ad1a2fa1fec080edd0c5a4ada9bc2c` |
