# Don re-approval of an amended spec, 2026-10-15

This decision record is approval evidence for the artifact index. It is not
part of the public mdBook. It amends the approval that
`docs/governance/decisions/2026-10-15-don-approvals.md` recorded for
`spec-h2-rustsec-2026-0258`.

- Approver: `repository-don-001` (role `Don`, scope `*`).
- Recorded at: `2026-10-15T09:24:25Z`.
- Channel: an interactive Claude Code session.
- Classification: `repository-visible`, unchanged.
- Independence exception: the Don owns this artifact and is the only
  registered human authority.

## What changed

After `h2` and `chacha20` were updated, `cargo audit` still reported nine
advisories in packages the lockfile already pinned:

- `quinn-proto` 0.11.16: RUSTSEC-2026-0338, 0339, 0340, 0341 and 0342.
- `rustls` 0.23.36: RUSTSEC-2026-0285, 0346, 0348 and 0349.

The same lockfile-only rules now also cover `quinn-proto` 0.11.19, `rustls`
0.23.46 and `rustls-webpki` 0.103.15.

## Don statement

The Don selected "Amend h2 spec and re-approve": add the three packages to
`spec-h2-rustsec-2026-0258` under the same lockfile-only rules, and record re-approval in a
dated addendum.

## Approved artifact

| Artifact ID | Path | SHA-256 at re-approval |
|---|---|---|
| `spec-h2-rustsec-2026-0258` | `_bmad-output/implementation-artifacts/spec-h2-rustsec-2026-0258.md` | `af14d5d0d7be2fa02335cc3d79873e613151726af674f790ca3c4bb91c41d2bd` |
