# Planning artifacts

This directory holds human-owned, durable planning decisions. Presence here is
not approval: `_bmad-output/index.yaml` is the canonical source for lifecycle,
scope, ownership, approval, effectiveness, supersession, archival, and
classification.

The `.yaml` governance registries intentionally use canonical JSON syntax so
the repository verifier can parse them with standard-library tooling. YAML-only
features and comments are forbidden in those registry files.

BMAD workflow frontmatter `type` and `status` are operational execution
metadata, distinct from the index `artifact_type` and lifecycle `status`
domains. An index-approved plan may therefore still be operationally
`in-review`; neither field silently changes the other.

Use a stable lowercase identifier made from letters, digits, and hyphens as both
the index ID and Markdown filename, for example `extension-plan-001.md`. Never
reuse an ID. Register every committed artifact at its canonical repository-
relative path; do not use absolute paths, traversal, symlinks, or duplicate
canonical targets.

Planning records may include full plans, product briefs, requirements,
architectures, epics, documented trivial exemptions, urgent-security incident
records, and schema migrations. Drafts remain drafts until the index contains
eligible human approval. A trivial exemption needs a rationale and approval from
an authorized repository maintainer other than the author, unless the Don
approves and evidences the independence exception. An urgent-security incident
needs security-owner or Don approval and a concrete retrospective deadline
calculated as the next five Monday-through-Friday calendar days in the
Europe/London timezone, without holiday exclusions.

Keep scratch work and transient generation outside `_bmad-output`; never use a
protected study as scratch space. Framework regeneration must not overwrite this
directory. Only `public` and `repository-visible` classifications are allowed.
Repository-visible means visible to everyone with repository access and excluded
from public documentation by default; it is not a confidentiality boundary.
Credentials, personal data, customer-confidential material, raw private
evidence, and live exploit material must remain outside Git.
