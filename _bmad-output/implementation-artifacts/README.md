# Implementation artifacts

This directory holds human-owned, durable implementation stories and related
change records. `_bmad-output/index.yaml`, not directory presence or filename,
determines lifecycle status, approval, effectiveness, supersession, archival,
scope, and classification.

The `.yaml` governance registries intentionally use canonical JSON syntax so
the repository verifier can parse them with standard-library tooling. YAML-only
features and comments are forbidden in those registry files.

BMAD workflow frontmatter `type` and `status` are operational execution
metadata, distinct from the index `artifact_type` and lifecycle `status`
domains. An index-approved plan may therefore still be operationally
`in-review`; neither field silently changes the other.

Use a stable lowercase identifier made from letters, digits, and hyphens as both
the index ID and Markdown filename, for example `extension-story-001.md`. Never
reuse an ID. Every child story must name its indexed parent. Parent plans, epics,
and child stories can remain effective together because their artifact types and
scopes are distinct.

Register every committed artifact at its canonical repository-relative path.
Paths must remain inside this directory and must not be absolute, traversing,
symlinked, missing, or duplicated after canonicalization. Draft approval and
transition fields remain `null`; approval and every later transition require the
authority, independence, evidence, and scope checks defined by the canonical
index and authority registry. Superseded and archived history remains committed
and indexed.

Keep transient build notes, generated framework output, and scratch work outside
this directory. Framework regeneration must not overwrite it, and protected
studies must never serve as scratch space. Only `public` and
`repository-visible` classifications are allowed. Repository-visible is not a
confidentiality or access-control boundary. Credentials, personal data,
customer-confidential material, raw private evidence, and live exploit material
must remain outside Git.
