# Repository agent instructions

## Default BMAD workflow

BMAD Method 6.11.0 is installed from the locked `bmad-method` package. Bundled
core is present, and BMM is the only selected optional module. BMAD is the
repository default for qualifying future extension planning and implementation.
Before planning or implementation, read
`_bmad-output/project-context.md`. Register durable work in
`_bmad-output/index.yaml` and resolve human approval through
`_bmad-output/authorities.yaml`. Run `scripts/verify-bmad.sh` before completion.

## Tool entrypoint

For Codex, use the tracked `.agents/skills/bmad-*` launchers. Use `bmad-help`
when routing is unclear.

## Repository authority and workflow sources

`Cargo.toml`, `ARCHITECTURE.md`, `SECURITY.md`,
`studies/rust-agent-v1/README.md`, `studies/rust-agent-pilot/README.md`,
`docs/book.toml`, `docs/src/SUMMARY.md`, and `.github/workflows/ci.yml` are
authoritative over generated BMAD guidance. Apply each within its documented
scope. If authoritative sources appear to conflict, stop and request
repository-maintainer review; generated guidance must not choose between them.

Use `_bmad/_config/manifest.yaml` as the installation manifest and
`_bmad/_config/skill-manifest.csv` as the canonical source for skill and
workflow identities and generation-provenance source paths. Paths in the skill
and file manifests are not deployed `_bmad` runtime targets. Each tracked
`bmad-*/SKILL.md` under both launcher roots is a runtime tool entrypoint. Only
when an entrypoint explicitly renders or resolves a workflow, follow the single
workflow path it returns.

## Workflow tiers

Apply these tiers in order without reordering or skipping a higher tier:

1. Protected-study prohibition. It has no exemption.
2. Urgent security procedure. Create a minimal incident record approved by an
   eligible `security-owner` or `Don` from the authority registry. The
   retrospective deadline is the next five Monday-through-Friday calendar days
   in `Europe/London`, with no holiday exclusions. After validation against that
   rule, the recorded deadline is authoritative. The incident record is
   temporary routing; a retrospective full-BMM artifact is required by that
   recorded deadline.
3. Mandatory full BMM planning for features, behavioral changes, architectural
   changes, public APIs, dependency changes, security-sensitive changes, and
   cross-crate work.
4. Trivial exemption only for typo-only, formatting-only, or mechanically
   generated-only changes when no mandatory category applies. Record a rationale
   and obtain approval from an eligible `repository-maintainer` who is not the
   author, unless the registry's Don explicitly approves and evidences the
   independence exception.

Information classification is human-reviewed only. The allowed values are
`public` and `repository-visible`. Repository-visible means visible to everyone
with repository access and excluded from public documentation by default; it is
not a confidentiality or access-control boundary.

## Protected studies

BMAD must not mutate or use `studies/rust-agent-v1` or
`studies/rust-agent-pilot` as scratch space. Automatically apply the same
protection to every future study whose protocol declares itself frozen, locked,
or pre-registered. A change to any protected task, grader, prompt, reference
patch, model, effort, image, budget, retry policy, endpoint, publication gate,
or other frozen term requires a new study.

## Data and publication boundaries

Do not place credentials, secrets, personal data, customer-confidential
information, live exploit material, raw private evidence, or anything needing
restricted storage in Git or BMAD output.

`_bmad` and `_bmad-output` are internal by default. Do not add them to public
mdBook navigation or links, documentation copy rules, or rendered output unless
publication is explicitly reviewed. `docs/book.toml` and
`docs/src/SUMMARY.md` define the public mdBook boundary.

## Upgrade policy

Never run the installer or regeneration in the working checkout. An exact stable
patch update requires renewed provenance review, regeneration and comparison in
a disposable clean clone, generated-only promotion, human review, and full
verification. Any minor or major update requires a new approved plan. Never
overwrite human-owned governance, project context, durable artifacts, or shared
configuration.

## Machine policy

The following marker-delimited JSON is the canonical cross-tool policy mirror.
Keep the complete marker, JSON, and marker bytes identical in `AGENTS.md` and
`CLAUDE.md`.

<!-- BMAD-GOVERNANCE-POLICY:START -->
{
  "artifact_governance": {
    "allowed_classifications": [
      "public",
      "repository-visible"
    ],
    "authorities": "_bmad-output/authorities.yaml",
    "classification_decision": "human-reviewed-only",
    "index": "_bmad-output/index.yaml",
    "repository_visible_meaning": "visible-to-all-repository-readers-not-a-confidentiality-or-access-control-boundary"
  },
  "authoritative_sources": {
    "conflict_action": "stop-and-request-repository-maintainer-review",
    "paths": [
      "Cargo.toml",
      "ARCHITECTURE.md",
      "SECURITY.md",
      "studies/rust-agent-v1/README.md",
      "studies/rust-agent-pilot/README.md",
      "docs/book.toml",
      "docs/src/SUMMARY.md",
      ".github/workflows/ci.yml"
    ],
    "precedence": "repository-authorities-override-generated-bmad-guidance"
  },
  "bmad": {
    "default_intent": "qualifying-future-extension-planning-and-implementation",
    "installation_manifest": "_bmad/_config/manifest.yaml",
    "installer": {
      "locked": true,
      "name": "bmad-method",
      "version": "6.11.0"
    },
    "lock": "bmad.lock.json",
    "method": "BMAD Method",
    "module": {
      "id": "bmm",
      "name": "BMad Method",
      "only_selected_optional_module": true,
      "version": "6.11.0"
    },
    "workflow_manifest": "_bmad/_config/skill-manifest.csv",
    "workflow_source": {
      "entrypoint_filename": "SKILL.md",
      "entrypoint_pattern": "bmad-*/SKILL.md",
      "entrypoint_role": "tool-entrypoint",
      "identity_and_path_source": "_bmad/_config/skill-manifest.csv",
      "manifest_path_semantics": "generation-provenance-not-deployed-runtime-targets",
      "resolution_rule": "when-entrypoint-explicitly-renders-or-resolves-a-workflow-follow-the-single-workflow-path-it-returns"
    }
  },
  "forbidden_data": [
    "credentials",
    "secrets",
    "personal-data",
    "customer-confidential-information",
    "live-exploit-material",
    "raw-private-evidence",
    "restricted-storage-material"
  ],
  "launchers": {
    "claude-code": {
      "prefix": "bmad-",
      "root": ".claude/skills"
    },
    "codex": {
      "prefix": "bmad-",
      "root": ".agents/skills"
    }
  },
  "policy_schema_version": 1,
  "project_context": "_bmad-output/project-context.md",
  "publication": {
    "forbidden_without_review": [
      "navigation",
      "links",
      "copy-rules",
      "rendered-output"
    ],
    "internal_roots": [
      "_bmad",
      "_bmad-output"
    ],
    "public_book_config": "docs/book.toml",
    "public_navigation": "docs/src/SUMMARY.md",
    "public_source_root": "docs/src",
    "review_required": true
  },
  "studies": {
    "current_protected_roots": [
      "studies/rust-agent-v1",
      "studies/rust-agent-pilot"
    ],
    "future_marker_keywords": [
      "frozen",
      "locked",
      "pre-registered"
    ],
    "future_marker_rule": "protocol-declaration-automatically-protects-study",
    "mutation_allowed": false,
    "new_study_required_for_changes_to": [
      "task",
      "grader",
      "prompt",
      "reference-patch",
      "model",
      "effort",
      "image",
      "budget",
      "retry-policy",
      "endpoint",
      "publication-gate",
      "other-frozen-term"
    ],
    "scratch_space_allowed": false
  },
  "tiers": [
    {
      "exemption_allowed": false,
      "id": "protected-study-prohibition",
      "order": 1,
      "rule": "protected-studies-cannot-be-mutated-or-used-as-scratch-space"
    },
    {
      "approver_roles": [
        "security-owner",
        "Don"
      ],
      "id": "urgent-security-procedure",
      "order": 2,
      "record": "minimal-incident-record",
      "retrospective_deadline": {
        "business_days": 5,
        "calculation": "next-five-Monday-through-Friday-calendar-days",
        "holiday_exclusions": false,
        "recorded_deadline_authoritative_after_validation": true,
        "timezone": "Europe/London"
      },
      "retrospective_full_bmm_artifact_due_by_recorded_deadline": true,
      "temporary_routing": true
    },
    {
      "categories": [
        "features",
        "behavioral-changes",
        "architectural-changes",
        "public-apis",
        "dependency-changes",
        "security-sensitive-changes",
        "cross-crate-work"
      ],
      "id": "mandatory-full-bmm-planning",
      "order": 3,
      "planning": "full-BMM"
    },
    {
      "approver_independent_of_author": true,
      "approver_role": "repository-maintainer",
      "don_exception": {
        "allowed": true,
        "evidence_required": true,
        "explicit_approval_required": true
      },
      "eligible_changes": [
        "typo-only",
        "formatting-only",
        "mechanically-generated-only"
      ],
      "id": "trivial-exemption",
      "mandatory_category_must_not_apply": true,
      "order": 4,
      "rationale_required": true
    }
  ],
  "upgrades": {
    "human_owned_paths_must_not_be_overwritten": [
      "_bmad-output/authorities.yaml",
      "_bmad-output/implementation-artifacts",
      "_bmad-output/index.yaml",
      "_bmad-output/planning-artifacts",
      "_bmad-output/project-context.md",
      "_bmad/custom/config.toml"
    ],
    "major_update_requires_new_approved_plan": true,
    "minor_update_requires_new_approved_plan": true,
    "stable_patch_update_must_be_exact": true,
    "stable_patch_update_steps": [
      "renewed-provenance-review",
      "disposable-clean-clone-regeneration-and-comparison",
      "generated-only-promotion",
      "human-review",
      "full-verification"
    ],
    "working_checkout_installer_or_regeneration_allowed": false
  },
  "verifier": "scripts/verify-bmad.sh"
}
<!-- BMAD-GOVERNANCE-POLICY:END -->
