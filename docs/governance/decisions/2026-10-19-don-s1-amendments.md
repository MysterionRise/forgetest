# Don decisions amending S1 agent-images, 2026-10-19

This decision record is approval evidence for the artifact index. It is not
part of the public mdBook.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-19T00:14:01Z`.
- Channel: interactive Claude Code session, at the bmad-build step-3 Ask First
  gate for `spec-agent-images`.
- Classification: `repository-visible`.
- Prior approval: `2026-10-18-don-approval-agent-images-and-live-spike.md`.
- Independence exception: the Don owns this artifact and is the only
  registered human authority.

## Findings put to the Don

1. The checksum-verified Codex package also ships `bin/codex-code-mode-host`,
   `codex-path/rg`, `codex-resources/bwrap` and a voice runtime. With only
   `bin/codex` installed, every `codex exec` reports that Code Mode is
   unavailable, and the vendor sandbox is absent.
2. The lock's `cli_version` includes container stderr. A lock made on arm64
   would not verify on the amd64 runner. Fixing this requires a Rust change,
   which the spec lists as Ask First.
3. On macOS, the default registry port 5000 is held by AirPlay Receiver.

## Options put to the Don

The Don chose one option per question. The option descriptions shown were:

1. Codex package. Selected "Full package as shipped (Recommended)": "Extract the
   whole verified package to /opt/codex and symlink /usr/local/bin/codex to it.
   This matches what an npm install provides. The lock SHA still covers
   bin/codex, and the image digest covers the helpers." Not chosen: "Helpers
   only, no voice", "Keep codex binary only".
2. `cli_version`. Selected "Fold into S3 (Recommended)": "S3 already changes
   the lock (v2) and is a Rust spec, and nothing uses a lock before S5. S1
   stays Rust-free, as approved." Not chosen: "Fix now in S1".
3. Port. Selected "Default to localhost:5001 (Recommended)": "It works out of
   the box on Macs; kind uses 5001 for the same reason. This amends one frozen
   matrix row, which you'd be approving here." Not chosen: "Keep 5000, accept
   override".

## Don statements and interpretations

1. "Full package as shipped". The Codex image installs the whole verified
   package, unmodified, at `/opt/codex-0.163.0`, with `/usr/local/bin/codex`
   as a symlink. The lock's `executable_sha256` still covers `bin/codex`. The
   image digest covers the helpers.
2. "Fold into S3". Recording `cli_version` from stdout only becomes part of
   S3's scope. S1 stays free of Rust product changes.
3. "Default to localhost:5001". This approves amending the frozen Build row
   of the I/O matrix from `localhost:5000` to `localhost:5001`.

## Approved artifact

The SHA-256 below was computed over the amended spec at the time of these
decisions. Later workflow edits to the same file (task checkboxes, `status`)
are not part of the approved content.

| Artifact ID | Path | SHA-256 at approval |
|---|---|---|
| `spec-agent-images` | `_bmad-output/implementation-artifacts/spec-agent-images.md` | `6d7fb5ed5d9b4e8f1268f7cacbf122fb5afe02919b7ba8c78aace8744e84a812` |
