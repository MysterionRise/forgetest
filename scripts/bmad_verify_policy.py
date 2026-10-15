"""Launcher manifests, repository instructions, and canonical machine policy."""

from __future__ import annotations

import csv
from pathlib import Path

from bmad_verify_common import (
    ID_RE,
    LOCKED_VERSION,
    POLICY_END,
    POLICY_START,
    SHA256_RE,
    abort,
    canonical_path,
    checked_target,
    exact_structure,
    load_json_bytes,
    sha256_file,
)


FORBIDDEN_DATA = [
    "credentials", "secrets", "personal-data", "customer-confidential-information",
    "live-exploit-material", "raw-private-evidence", "restricted-storage-material",
]

PUBLICATION_POLICY = {
    "forbidden_without_review": ["navigation", "links", "copy-rules", "rendered-output"],
    "internal_roots": ["_bmad", "_bmad-output"],
    "public_book_config": "docs/book.toml",
    "public_navigation": "docs/src/SUMMARY.md",
    "public_source_root": "docs/src",
    "review_required": True,
}

STUDY_POLICY = {
    "current_protected_roots": ["studies/rust-agent-v1", "studies/rust-agent-pilot"],
    "future_marker_keywords": ["frozen", "locked", "pre-registered"],
    "future_marker_rule": "protocol-declaration-automatically-protects-study",
    "mutation_allowed": False,
    "new_study_required_for_changes_to": [
        "task", "grader", "prompt", "reference-patch", "model", "effort", "image",
        "budget", "retry-policy", "endpoint", "publication-gate", "other-frozen-term",
    ],
    "scratch_space_allowed": False,
}

UPGRADE_POLICY = {
    "human_owned_paths_must_not_be_overwritten": [
        "_bmad-output/authorities.yaml", "_bmad-output/implementation-artifacts",
        "_bmad-output/index.yaml", "_bmad-output/planning-artifacts",
        "_bmad-output/project-context.md", "_bmad/custom/config.toml",
    ],
    "major_update_requires_new_approved_plan": True,
    "minor_update_requires_new_approved_plan": True,
    "stable_patch_update_must_be_exact": True,
    "stable_patch_update_steps": [
        "renewed-provenance-review", "disposable-clean-clone-regeneration-and-comparison",
        "generated-only-promotion", "human-review", "full-verification",
    ],
    "working_checkout_installer_or_regeneration_allowed": False,
}

TIER_POLICY = [
    {
        "exemption_allowed": False,
        "id": "protected-study-prohibition",
        "order": 1,
        "rule": "protected-studies-cannot-be-mutated-or-used-as-scratch-space",
    },
    {
        "approver_roles": ["security-owner", "Don"],
        "id": "urgent-security-procedure",
        "order": 2,
        "record": "minimal-incident-record",
        "retrospective_deadline": {
            "business_days": 5,
            "calculation": "next-five-Monday-through-Friday-calendar-days",
            "holiday_exclusions": False,
            "recorded_deadline_authoritative_after_validation": True,
            "timezone": "Europe/London",
        },
        "retrospective_full_bmm_artifact_due_by_recorded_deadline": True,
        "temporary_routing": True,
    },
    {
        "categories": [
            "features", "behavioral-changes", "architectural-changes", "public-apis",
            "dependency-changes", "security-sensitive-changes", "cross-crate-work",
        ],
        "id": "mandatory-full-bmm-planning",
        "order": 3,
        "planning": "full-BMM",
    },
    {
        "approver_independent_of_author": True,
        "approver_role": "repository-maintainer",
        "don_exception": {
            "allowed": True,
            "evidence_required": True,
            "explicit_approval_required": True,
        },
        "eligible_changes": ["typo-only", "formatting-only", "mechanically-generated-only"],
        "id": "trivial-exemption",
        "mandatory_category_must_not_apply": True,
        "order": 4,
        "rationale_required": True,
    },
]

ARTIFACT_GOVERNANCE_POLICY = {
    "allowed_classifications": ["public", "repository-visible"],
    "authorities": "_bmad-output/authorities.yaml",
    "classification_decision": "human-reviewed-only",
    "index": "_bmad-output/index.yaml",
    "repository_visible_meaning": (
        "visible-to-all-repository-readers-not-a-confidentiality-or-access-control-boundary"
    ),
}

AUTHORITATIVE_SOURCES_POLICY = {
    "conflict_action": "stop-and-request-repository-maintainer-review",
    "paths": [
        "Cargo.toml", "ARCHITECTURE.md", "SECURITY.md",
        "studies/rust-agent-v1/README.md", "studies/rust-agent-pilot/README.md",
        "docs/book.toml", "docs/src/SUMMARY.md", ".github/workflows/ci.yml",
    ],
    "precedence": "repository-authorities-override-generated-bmad-guidance",
}

BMAD_POLICY = {
    "default_intent": "qualifying-future-extension-planning-and-implementation",
    "installation_manifest": "_bmad/_config/manifest.yaml",
    "installer": {"locked": True, "name": "bmad-method", "version": LOCKED_VERSION},
    "lock": "bmad.lock.json",
    "method": "BMAD Method",
    "module": {
        "id": "bmm", "name": "BMad Method",
        "only_selected_optional_module": True, "version": LOCKED_VERSION,
    },
    "workflow_manifest": "_bmad/_config/skill-manifest.csv",
    "workflow_source": {
        "entrypoint_filename": "SKILL.md",
        "entrypoint_pattern": "bmad-*/SKILL.md",
        "entrypoint_role": "tool-entrypoint",
        "identity_and_path_source": "_bmad/_config/skill-manifest.csv",
        "manifest_path_semantics": "generation-provenance-not-deployed-runtime-targets",
        "resolution_rule": (
            "when-entrypoint-explicitly-renders-or-resolves-a-workflow-"
            "follow-the-single-workflow-path-it-returns"
        ),
    },
}

LAUNCHER_POLICY = {
    "claude-code": {"prefix": "bmad-", "root": ".claude/skills"},
    "codex": {"prefix": "bmad-", "root": ".agents/skills"},
}

MACHINE_POLICY = {
    "artifact_governance": ARTIFACT_GOVERNANCE_POLICY,
    "authoritative_sources": AUTHORITATIVE_SOURCES_POLICY,
    "bmad": BMAD_POLICY,
    "forbidden_data": FORBIDDEN_DATA,
    "launchers": LAUNCHER_POLICY,
    "policy_schema_version": 1,
    "project_context": "_bmad-output/project-context.md",
    "publication": PUBLICATION_POLICY,
    "studies": STUDY_POLICY,
    "tiers": TIER_POLICY,
    "upgrades": UPGRADE_POLICY,
    "verifier": "scripts/verify-bmad.sh",
}


class PolicyVerifierMixin:
    def verify_skill_manifest(self, launcher_files: dict[str, str]) -> None:
        rows = self.read_csv(
            "_bmad/_config/skill-manifest.csv",
            {"canonicalId", "name", "description", "module", "path"},
            "skill manifest",
        )
        ids: set[str] = set()
        source_paths: set[str] = set()
        skills: list[tuple[str, str]] = []
        for row in rows:
            canonical_id = row.get("canonicalId", "")
            if not ID_RE.fullmatch(canonical_id) or canonical_id in ids:
                abort("duplicate or invalid workflow identity in skill manifest")
            ids.add(canonical_id)
            module = row.get("module")
            if module not in {"core", "bmm"}:
                abort(f"invalid module in skill manifest: {canonical_id}")
            source = canonical_path(row.get("path"), "skill manifest")
            if source in source_paths or not source.startswith(f"_bmad/{module}/"):
                abort(f"duplicate or invalid skill manifest path: {source}")
            if (
                row.get("name") != canonical_id
                or not row.get("description", "").strip()
                or not source.endswith(f"/{canonical_id}/SKILL.md")
            ):
                abort(f"invalid skill manifest identity mapping: {canonical_id}")
            source_paths.add(source)
            skills.append((canonical_id, source))

        launcher_ids = {
            relative.split("/", 1)[0]
            for relative in launcher_files
            if relative.endswith("/SKILL.md")
        }
        if launcher_ids != ids:
            abort("launcher workflow identity inventory does not match skill manifest")
        expected_count = self.lock.get("generation", {}).get("result", {}).get(
            "skills_per_launcher"
        )
        if expected_count != len(ids):
            abort("launcher workflow identity count does not match lock")
        self.verify_file_manifest(skills, launcher_files)

    def verify_file_manifest(
        self, skills: list[tuple[str, str]], launcher_files: dict[str, str]
    ) -> None:
        """Reconcile one provenance manifest across both deployed launcher trees."""
        rows = self.read_csv(
            "_bmad/_config/files-manifest.csv",
            {"type", "name", "module", "path", "hash"},
            "file manifest",
        )
        file_rows: dict[str, dict[str, str]] = {}
        for row in rows:
            relative = canonical_path(row.get("path"), "file manifest")
            digest = row.get("hash", "")
            if (
                relative.startswith("_bmad/")
                or relative in file_rows
                or not row.get("module", "").strip()
                or SHA256_RE.fullmatch(digest) is None
            ):
                abort("invalid generation-provenance file manifest row")
            file_rows[relative] = row

        expected: dict[str, str] = {}
        excluded = set(
            self.lock.get("generation", {})
            .get("promotion_sanitization", {})
            .get("excluded_transient_files", [])
        )
        for skill_id, source in skills:
            source_relative = source.removeprefix("_bmad/")
            source_root = str(Path(source_relative).parent).replace("\\", "/")
            matched = False
            for relative, row in file_rows.items():
                if relative != source_relative and not relative.startswith(source_root + "/"):
                    continue
                matched = True
                suffix = relative[len(source_root) + 1 :]
                launcher_relative = f"{skill_id}/{suffix}"
                agent_path = f".agents/skills/{launcher_relative}"
                claude_path = f".claude/skills/{launcher_relative}"
                if agent_path in excluded or claude_path in excluded:
                    if not {agent_path, claude_path}.issubset(excluded):
                        abort("file manifest transient launcher exclusions diverge")
                    continue
                if launcher_relative in expected:
                    abort("duplicate file manifest skill mapping")
                expected[launcher_relative] = row["hash"]
            if not matched or f"{skill_id}/SKILL.md" not in expected:
                abort(f"file manifest skill mapping is missing: {skill_id}")
        if set(expected) != set(launcher_files):
            abort("file manifest skill mapping does not match launcher inventory")
        for relative, digest in expected.items():
            if launcher_files[relative] != digest:
                abort(f"file manifest launcher hash mismatch: .agents/skills/{relative}")
            claude = self.root / ".claude" / "skills" / relative
            if not claude.is_file() or sha256_file(claude) != digest:
                abort(f"file manifest launcher hash mismatch: .claude/skills/{relative}")

    def read_csv(
        self, relative: str, fields: set[str], label: str
    ) -> list[dict[str, str]]:
        path = checked_target(self.root, relative, label, require_file=True)
        try:
            with path.open("r", encoding="utf-8", newline="") as handle:
                reader = csv.DictReader(handle)
                rows = list(reader)
        except (OSError, UnicodeError, csv.Error) as exc:
            abort(f"invalid {label} CSV: {exc}")
        if not rows or set(rows[0]) != fields:
            abort(f"invalid {label} schema")
        return rows

    def verify_repository_instructions(self) -> None:
        sections: dict[str, str] = {}
        full_text: dict[str, str] = {}
        for name in ("AGENTS.md", "CLAUDE.md"):
            path = checked_target(self.root, name, "repository instruction", require_file=True)
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError):
                abort(f"unable to read shared artifact instruction surface: {name}")
            full_text[name] = text
            if text.count(POLICY_START) != 1 or text.count(POLICY_END) != 1:
                abort(f"repository instruction policy marker is invalid: {name}")
            start = text.index(POLICY_START)
            end = text.index(POLICY_END, start) + len(POLICY_END)
            sections[name] = text[start:end]
        if sections["AGENTS.md"] != sections["CLAUDE.md"]:
            abort("repository instruction authority precedence policy diverges")
        section = sections["AGENTS.md"]
        policy = load_json_bytes(
            section[section.index("\n") + 1 : section.rfind("\n")].encode(),
            "repository policy mirror",
        )
        if not isinstance(policy, dict):
            abort("invalid repository policy mirror")
        self.verify_policy_core(policy)
        prose = (
            "generation-provenance source paths",
            "not deployed `_bmad` runtime targets",
            "runtime tool entrypoint",
        )
        if any(token not in text for text in full_text.values() for token in prose):
            abort("repository instruction manifest path semantics are incomplete")
        self.verify_onboarding()

    def verify_policy_core(self, policy: dict[str, object]) -> None:
        if policy.get("policy_schema_version") != 1 or type(
            policy.get("policy_schema_version")
        ) is not int:
            abort("invalid repository policy schema")
        bmad = policy.get("bmad")
        workflow = bmad.get("workflow_source") if isinstance(bmad, dict) else None
        if (
            not isinstance(bmad, dict)
            or bmad.get("installer", {}).get("version") != LOCKED_VERSION
            or not isinstance(workflow, dict)
            or workflow.get("identity_and_path_source")
            != "_bmad/_config/skill-manifest.csv"
            or workflow.get("manifest_path_semantics")
            != "generation-provenance-not-deployed-runtime-targets"
        ):
            abort("repository instruction manifest path policy is invalid")
        governance = policy.get("artifact_governance")
        if (
            not isinstance(governance, dict)
            or governance.get("index") != "_bmad-output/index.yaml"
            or governance.get("authorities") != "_bmad-output/authorities.yaml"
            or policy.get("verifier") != "scripts/verify-bmad.sh"
        ):
            abort("repository instruction governance references are invalid")
        if policy.get("project_context") != "_bmad-output/project-context.md":
            abort("repository instruction context reference is invalid")
        authority = policy.get("authoritative_sources")
        if not exact_structure(authority, AUTHORITATIVE_SOURCES_POLICY):
            abort("repository instruction authority precedence is invalid")
        if not exact_structure(policy.get("tiers"), TIER_POLICY):
            abort("repository instruction workflow tiers are invalid")
        if not exact_structure(policy.get("forbidden_data"), FORBIDDEN_DATA):
            abort("repository instruction forbidden-data policy is invalid")
        if not exact_structure(policy.get("upgrades"), UPGRADE_POLICY):
            abort("repository instruction upgrade policy is invalid")
        if not exact_structure(policy.get("publication"), PUBLICATION_POLICY):
            abort("repository instruction publication policy is invalid")
        if not exact_structure(policy.get("studies"), STUDY_POLICY):
            abort("repository instruction protected-study policy is invalid")
        if not exact_structure(policy, MACHINE_POLICY):
            abort("repository instruction machine policy is invalid")

    def verify_onboarding(self) -> None:
        path = checked_target(
            self.root, "README.md", "BMAD onboarding", require_file=True
        )
        try:
            readme = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            abort("unable to read shared artifact instruction surface: README.md")
        required = (
            "BMAD contributor workflow", LOCKED_VERSION, "AGENTS.md", "CLAUDE.md",
            "_bmad-output/project-context.md", "_bmad-output/authorities.yaml",
            "_bmad-output/index.yaml", "scripts/verify-bmad.sh", "Minor or major updates",
            "public `docs/src` mdBook",
        )
        if any(value not in readme for value in required):
            abort("BMAD onboarding is incomplete")
