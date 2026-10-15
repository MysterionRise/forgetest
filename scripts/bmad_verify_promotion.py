"""Generated inventory, reproducibility, and repository-promotion proof checks."""

from __future__ import annotations

from typing import Any

from bmad_verify_common import SHA256_RE, abort, exact_structure
from bmad_verify_provenance import exact_fields, exact_integer


def sha256_value(value: Any) -> bool:
    return isinstance(value, str) and SHA256_RE.fullmatch(value) is not None


def string_list(value: Any) -> bool:
    return isinstance(value, list) and all(isinstance(item, str) for item in value)


class PromotionVerifierMixin:
    def verify_inventory_and_promotion_proofs(
        self,
        generation: dict[str, Any],
        clone: dict[str, Any],
        baseline: dict[str, Any],
        result: dict[str, Any],
    ) -> None:
        inventory = generation.get("inventory")
        inventory_fields = {
            "normalization", "digest", "raw_promotion_sha256",
            "normalized_reproducibility_sha256", "path_set_sha256", "path_count",
            "directory_count", "file_count", "generated_paths", "generated_files_sha256",
            "empty_directories", "symlinks", "special_files", "absolute_paths",
            "traversal_paths", "duplicate_canonical_targets",
            "excluded_container_directories", "human_owned_files",
            "human_owned_output_roots",
        }
        if not exact_fields(inventory, inventory_fields):
            abort("invalid generated inventory proof fields")
        assert isinstance(inventory, dict)
        if (
            not isinstance(inventory.get("normalization"), str)
            or not inventory["normalization"].strip()
            or not isinstance(inventory.get("digest"), str)
            or not inventory["digest"].strip()
            or not all(
                sha256_value(inventory.get(field))
                for field in (
                    "raw_promotion_sha256", "normalized_reproducibility_sha256",
                    "path_set_sha256",
                )
            )
            or any(
                inventory.get(field) != []
                for field in (
                    "empty_directories", "symlinks", "special_files", "absolute_paths",
                    "traversal_paths", "duplicate_canonical_targets",
                )
            )
            or inventory.get("excluded_container_directories")
            != [".agents", ".agents/skills", ".claude", ".claude/skills"]
        ):
            abort("invalid generated inventory proof fields")

        self.verify_filesystem_audits(generation)
        team_record = self.verify_promotion_sanitization(generation)
        self.verify_reproducibility(generation, clone)
        self.verify_repository_promotion(inventory, baseline, result, team_record)

    def verify_filesystem_audits(self, generation: dict[str, Any]) -> None:
        raw = generation.get("raw_installer_inventory")
        raw_fields = {
            "path_count", "directory_count", "file_count", "framework_files",
            "files_per_launcher",
        }
        if not exact_fields(raw, raw_fields) or not all(
            exact_integer(raw.get(field)) for field in raw_fields
        ):
            abort("invalid raw installer inventory")
        assert isinstance(raw, dict)
        if (
            raw["path_count"] != raw["directory_count"] + raw["file_count"]
            or raw["file_count"] != raw["framework_files"] + 2 * raw["files_per_launcher"]
        ):
            abort("invalid raw installer inventory")

        sanitized = generation.get("sanitized_filesystem_inventory")
        sanitized_fields = {
            "path_count", "directory_count", "file_count", "first_raw_sha256",
            "second_raw_sha256",
        }
        if (
            not exact_fields(sanitized, sanitized_fields)
            or not all(
                exact_integer(sanitized.get(field))
                for field in ("path_count", "directory_count", "file_count")
            )
            or not sha256_value(sanitized.get("first_raw_sha256"))
            or not sha256_value(sanitized.get("second_raw_sha256"))
            or sanitized["path_count"] != sanitized["directory_count"] + sanitized["file_count"]
        ):
            abort("invalid sanitized filesystem inventory")

        safety = generation.get("safety_audit")
        safety_fields = {
            "absolute_path_count", "credential_hit_count", "duplicate_portable_target_count",
            "machine_path_hit_count", "output_overlap_count", "special_file_count",
            "symlink_count", "traversal_path_count",
        }
        if not exact_fields(safety, safety_fields) or any(
            type(safety.get(field)) is not int or safety[field] != 0 for field in safety_fields
        ):
            abort("invalid generation safety audit")

    def verify_promotion_sanitization(self, generation: dict[str, Any]) -> dict[str, Any]:
        sanitization = generation.get("promotion_sanitization")
        fields = {
            "excluded_transient_files", "excluded_transient_reason",
            "removed_empty_transient_directories", "excluded_local_state_files",
            "excluded_local_state_reason", "team_override", "root_ignore_follow_up",
        }
        if not exact_fields(sanitization, fields):
            abort("invalid promotion sanitization proof")
        assert isinstance(sanitization, dict)
        for field in (
            "excluded_transient_files", "removed_empty_transient_directories",
            "excluded_local_state_files",
        ):
            if not string_list(sanitization.get(field)):
                abort("invalid promotion sanitization proof")
        if sanitization.get("excluded_local_state_files") != [
            "_bmad/config.user.toml", "_bmad/custom/config.user.toml"
        ] or not all(
            isinstance(sanitization.get(field), str) and sanitization[field].strip()
            for field in (
                "excluded_transient_reason", "excluded_local_state_reason",
                "root_ignore_follow_up",
            )
        ):
            abort("invalid promotion sanitization proof")
        team = sanitization.get("team_override")
        if (
            not exact_fields(team, {"path", "sha256", "settings", "classification"})
            or team.get("path") != "_bmad/custom/config.toml"
            or not sha256_value(team.get("sha256"))
            or not isinstance(team.get("settings"), dict)
            or team.get("classification") != "human-owned"
        ):
            abort("invalid human-owned team override proof")
        return team

    def verify_reproducibility(
        self, generation: dict[str, Any], clone: dict[str, Any]
    ) -> None:
        inventory = generation["inventory"]
        proof = generation.get("reproducibility")
        fields = {
            "install_count", "path_sets_match", "first_raw_promotion_sha256",
            "second_raw_promotion_sha256", "raw_differing_files", "normalization",
            "normalized_values", "normalized_value_count", "normalized_tree_sha256",
            "normalized_file_hashes_match", "no_other_byte_drift",
            "human_owned_identity_matches",
        }
        if not exact_fields(proof, fields):
            abort("generation reproducibility proof is missing")
        assert isinstance(proof, dict)
        first = proof.get("first_raw_promotion_sha256")
        second = proof.get("second_raw_promotion_sha256")
        differences = proof.get("raw_differing_files")
        normalized = proof.get("normalized_values")
        if (
            not exact_integer(proof.get("install_count"), 2)
            or proof["install_count"] != clone.get("comparison_install_count")
            or proof.get("path_sets_match") is not True
            or first != inventory.get("raw_promotion_sha256")
            or not sha256_value(second)
            or not string_list(differences)
            or bool(differences) != (first != second)
            or not isinstance(proof.get("normalization"), str)
            or not proof["normalization"].strip()
            or not string_list(normalized)
            or not exact_integer(proof.get("normalized_value_count"))
            or proof["normalized_value_count"] != len(normalized)
            or proof.get("normalized_tree_sha256")
            != inventory.get("normalized_reproducibility_sha256")
            or proof.get("normalized_file_hashes_match") is not True
            or proof.get("no_other_byte_drift") is not True
            or proof.get("human_owned_identity_matches") is not True
        ):
            abort("generation reproducibility proof is inconsistent")

    def verify_repository_promotion(
        self,
        inventory: dict[str, Any],
        baseline: dict[str, Any],
        result: dict[str, Any],
        team: dict[str, Any],
    ) -> None:
        promotion = self.lock.get("repository_promotion")
        fields = {
            "status", "method", "installer_executed_in_repository",
            "source_baseline_revision", "working_baseline_revision",
            "source_raw_generated_tree_sha256", "promoted_raw_generated_tree_sha256",
            "source_path_set_sha256", "promoted_path_set_sha256", "generated_file_count",
            "generated_file_hashes_match", "accepted_generated_hashes_unchanged",
            "human_owned_files", "launchers", "tracked_allowlist_patterns",
            "excluded_files_absent", "pre_existing_claude_local_state_preserved",
            "identifier_review", "symlink_count", "special_file_count",
        }
        if not exact_fields(promotion, fields):
            abort("repository promotion proof is incomplete")
        assert isinstance(promotion, dict)
        raw_digest = inventory["raw_promotion_sha256"]
        path_digest = inventory["path_set_sha256"]
        if (
            promotion.get("status") != "verified"
            or promotion.get("method")
            != "byte-for-byte-copy-from-accepted-sanitized-disposable-tree"
            or promotion.get("installer_executed_in_repository") is not False
            or promotion.get("source_baseline_revision") != baseline.get("revision")
            or promotion.get("working_baseline_revision") != baseline.get("revision")
            or promotion.get("source_raw_generated_tree_sha256") != raw_digest
            or promotion.get("promoted_raw_generated_tree_sha256") != raw_digest
            or promotion.get("source_path_set_sha256") != path_digest
            or promotion.get("promoted_path_set_sha256") != path_digest
            or not exact_integer(promotion.get("generated_file_count"))
            or promotion["generated_file_count"] != len(self.generated_hashes)
            or promotion.get("generated_file_hashes_match") is not True
            or promotion.get("accepted_generated_hashes_unchanged") is not True
        ):
            abort("repository promotion proof is inconsistent")
        human = promotion.get("human_owned_files")
        expected_human = [{"path": team["path"], "sha256": team["sha256"], "hash_matches": True}]
        if not exact_structure(human, expected_human):
            abort("repository promotion human-owned proof is invalid")
        self.verify_promoted_launchers(promotion, result)

    def verify_promoted_launchers(
        self, promotion: dict[str, Any], result: dict[str, Any]
    ) -> None:
        launchers = promotion.get("launchers")
        if not exact_fields(launchers, {"codex", "claude-code", "parity", "tree_sha256"}):
            abort("repository promotion launcher proof is invalid")
        assert isinstance(launchers, dict)
        for tool, root in (("codex", ".agents/skills"), ("claude-code", ".claude/skills")):
            expected = {
                "root": root,
                "skill_count": result.get("skills_per_launcher"),
                "file_count": result.get("files_per_launcher"),
            }
            if not exact_structure(launchers.get(tool), expected):
                abort("repository promotion launcher proof is invalid")
        identifier = promotion.get("identifier_review")
        if (
            launchers.get("parity") is not True
            or launchers.get("tree_sha256") != result.get("launcher_tree_sha256")
            or promotion.get("tracked_allowlist_patterns")
            != [
                "_bmad/** except _bmad/config.user.toml, _bmad/custom/config.toml ownership, and *.user.toml local state",
                ".agents/skills/bmad-*/**",
                ".claude/skills/bmad-*/**",
            ]
            or promotion.get("excluded_files_absent")
            != [
                "_bmad/config.user.toml", "_bmad/custom/config.user.toml",
                "**/__pycache__/**", "**/*.pyc",
            ]
            or promotion.get("pre_existing_claude_local_state_preserved") is not True
            or not exact_fields(
                identifier,
                {
                    "gangsta_contract_identifier_matches", "upstream_generic_bmad_matches",
                    "upstream_generic_values", "disposition",
                },
            )
            or not exact_integer(identifier.get("gangsta_contract_identifier_matches"))
            or not exact_integer(identifier.get("upstream_generic_bmad_matches"))
            or not string_list(identifier.get("upstream_generic_values"))
            or not isinstance(identifier.get("disposition"), str)
            or type(promotion.get("symlink_count")) is not int
            or promotion.get("symlink_count") != 0
            or type(promotion.get("special_file_count")) is not int
            or promotion.get("special_file_count") != 0
        ):
            abort("repository promotion proof is inconsistent")
