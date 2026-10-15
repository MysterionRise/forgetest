"""Dependency, generation, reproducibility, and promotion proof verification."""

from __future__ import annotations

import base64
import binascii
import hashlib
import re
from typing import Any

from bmad_verify_common import (
    LOCKED_VERSION,
    SHA256_RE,
    SHA512_RE,
    SHA512_BYTE_LENGTH,
    abort,
    canonical_path,
    exact_structure,
    normalized_json_bytes,
)


CARGO_LOCKFILE_VERSION = 3
MINIMUM_REPRODUCTION_INSTALLS = 2


def exact_fields(value: Any, fields: set[str]) -> bool:
    return isinstance(value, dict) and set(value) == fields


def exact_integer(value: Any, minimum: int = 0) -> bool:
    return type(value) is int and value >= minimum


def valid_sha512_sri(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("sha512-"):
        return False
    try:
        decoded = base64.b64decode(value[7:], validate=True)
    except (binascii.Error, ValueError):
        return False
    return len(decoded) == SHA512_BYTE_LENGTH


class ProvenanceVerifierMixin:
    def verify_lock_contract_schema(self) -> None:
        """Validate one lock schema in contract-significant fail-fast order."""
        top_fields = {
            "schema_version", "installer", "runtime", "platform", "scope", "baseline",
            "dependencies", "dependency_audit", "generation", "repository_promotion",
            "version_policy",
        }
        if set(self.lock) != top_fields:
            abort("invalid BMAD lock declared fields")
        policy = {
            "locked": LOCKED_VERSION,
            "patch_review_series": "6.11.x",
            "automatic_updates": False,
            "minor_or_major_update": "requires-new-approved-plan",
        }
        if not exact_structure(self.lock.get("version_policy"), policy):
            abort("invalid BMAD lock version policy")

        installer = self.lock.get("installer")
        installer_fields = {
            "package", "version", "registry", "tarball", "integrity", "tarball_sha512",
            "registry_shasum_sha1", "registry_signature_keyid", "registry_attestation",
            "file_count", "unpacked_size_bytes", "license", "entry_point", "executables",
            "node_requirement", "lifecycle",
        }
        if not exact_fields(installer, installer_fields):
            abort("invalid installer provenance fields in BMAD lock")
        assert isinstance(installer, dict)
        package_url = "https://registry.npmjs.org/bmad-method/-/bmad-method-6.11.0.tgz"
        if (
            installer.get("registry") != "https://registry.npmjs.org"
            or installer.get("tarball") != package_url
            or installer.get("license") != "MIT"
            or installer.get("entry_point") != "tools/installer/bmad-cli.js"
            or installer.get("executables")
            != {
                "bmad": "tools/installer/bmad-cli.js",
                "bmad-method": "tools/installer/bmad-cli.js",
            }
            or installer.get("node_requirement") != ">=20.12.0"
        ):
            abort("invalid immutable installer entry point or identity")
        if (
            not re.fullmatch(r"[0-9a-f]{40}", str(installer.get("registry_shasum_sha1", "")))
            or not str(installer.get("registry_signature_keyid", "")).startswith("SHA256:")
            or installer.get("registry_attestation")
            != "https://registry.npmjs.org/-/npm/v1/attestations/bmad-method@6.11.0"
            or not exact_integer(installer.get("file_count"), 1)
            or not exact_integer(installer.get("unpacked_size_bytes"), 1)
        ):
            abort("invalid installer provenance metadata in BMAD lock")
        lifecycle = installer.get("lifecycle")
        if (
            not exact_fields(lifecycle, {"preinstall", "install", "postinstall", "prepare"})
            or any(lifecycle.get(name) is not None for name in ("preinstall", "install", "postinstall"))
            or not isinstance(lifecycle.get("prepare"), str)
            or not lifecycle["prepare"].strip()
        ):
            abort("invalid installer lifecycle audit")

        runtime = self.lock.get("runtime")
        platform = self.lock.get("platform")
        scope = self.lock.get("scope")
        if not exact_fields(runtime, {"node", "npm"}):
            abort("invalid locked runtime metadata")
        if not exact_fields(platform, {"os", "os_version", "architecture"}) or not all(
            isinstance(platform.get(key), str) and platform[key].strip()
            for key in ("os", "os_version", "architecture")
        ):
            abort("invalid locked platform metadata")
        if not exact_structure(
            scope,
            {"modules": ["bmm"], "tools": ["codex", "claude-code"], "output_root": "_bmad-output"},
        ):
            abort("invalid immutable BMAD output root or installation scope")
        audit = {
            "environment": "fresh-disposable-directory-outside-repository",
            "lifecycle_scripts": "disabled",
            "installer_executed": False,
            "repository_mutated": False,
        }
        if not exact_structure(self.lock.get("dependency_audit"), audit):
            abort("invalid dependency audit proof")

    def verify_dependency_payload(self) -> None:
        """Validate one dependency graph in contract-significant fail-fast order."""
        dependencies = self.lock.get("dependencies")
        fields = {
            "lockfile_version", "production_only", "direct_count", "resolved_count",
            "resolution_problems", "install_script_packages", "canonical_payload_sha512",
            "declared", "nodes",
        }
        if not isinstance(dependencies, dict):
            abort("missing locked dependency graph")
        if "resolution_problems" not in dependencies:
            abort("invalid dependency resolution audit")
        if "install_script_packages" not in dependencies:
            abort("invalid dependency install-script audit")
        if set(dependencies) != fields:
            abort("missing locked dependency graph")
        nodes = dependencies.get("nodes")
        declared = dependencies.get("declared")
        if (
            dependencies.get("lockfile_version") != CARGO_LOCKFILE_VERSION
            or type(dependencies.get("lockfile_version")) is not int
            or dependencies.get("production_only") is not True
            or not isinstance(nodes, list)
            or not isinstance(declared, dict)
        ):
            abort("invalid locked dependency graph")
        if (
            not exact_integer(dependencies.get("direct_count"))
            or dependencies["direct_count"] != len(declared)
            or not exact_integer(dependencies.get("resolved_count"))
            or dependencies["resolved_count"] != len(nodes)
        ):
            abort("locked dependency count is invalid")
        if not all(
            isinstance(name, str) and name and isinstance(spec, str) and spec
            for name, spec in declared.items()
        ):
            abort("invalid direct dependency schema")

        paths: list[str] = []
        for node in nodes:
            if not exact_fields(
                node, {"dependencies", "integrity", "optional_dependencies", "path", "version"}
            ):
                abort("invalid dependency node schema")
            assert isinstance(node, dict)
            raw_path = node.get("path")
            if (
                not isinstance(raw_path, str)
                or raw_path != canonical_path(raw_path, "dependency node")
                or not raw_path.startswith("node_modules/")
                or not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?", str(node.get("version", "")))
            ):
                abort("invalid dependency node schema")
            paths.append(raw_path)
            if not valid_sha512_sri(node.get("integrity")):
                abort("invalid dependency integrity SRI")
            for edge_field in ("dependencies", "optional_dependencies"):
                edges = node.get(edge_field)
                if not isinstance(edges, dict) or not all(
                    isinstance(name, str) and name and isinstance(spec, str) and spec
                    for name, spec in edges.items()
                ):
                    abort("invalid dependency edge schema")
        if paths != sorted(set(paths)):
            abort("dependency node paths are not unique and canonical")
        for name in declared:
            if not any(path == f"node_modules/{name}" or path.endswith(f"/node_modules/{name}") for path in paths):
                abort("direct dependency is absent from resolved graph")
        for node in nodes:
            for edge_field in ("dependencies", "optional_dependencies"):
                for name in node[edge_field]:
                    if not any(path == f"node_modules/{name}" or path.endswith(f"/node_modules/{name}") for path in paths):
                        abort("dependency edge is absent from resolved graph")

        if type(dependencies.get("resolution_problems")) is not int or dependencies.get(
            "resolution_problems"
        ) != 0:
            abort("invalid dependency resolution audit")
        if dependencies.get("install_script_packages") != []:
            abort("locked dependency graph contains install scripts")
        expected = dependencies.get("canonical_payload_sha512")
        if not isinstance(expected, str) or not SHA512_RE.fullmatch(expected):
            abort("invalid dependency payload SHA-512")
        actual = hashlib.sha512(normalized_json_bytes(nodes)).hexdigest()
        if actual != expected:
            abort("locked dependency graph digest mismatch")

    def verify_generation_and_promotion_claims(self) -> None:
        """Validate cross-field generation claims as one ordered proof chain."""
        runtime = self.lock.get("runtime")
        platform = self.lock.get("platform")
        generation = self.lock.get("generation")
        baseline = self.lock.get("baseline")
        scope = self.lock.get("scope")
        installer = self.lock.get("installer")
        dependencies = self.lock.get("dependencies")
        if not all(
            isinstance(value, dict)
            for value in (runtime, platform, generation, baseline, scope, installer, dependencies)
        ):
            abort("invalid generation environment metadata")
        assert isinstance(runtime, dict)
        assert isinstance(platform, dict)
        assert isinstance(generation, dict)
        assert isinstance(baseline, dict)
        assert isinstance(scope, dict)
        assert isinstance(installer, dict)
        assert isinstance(dependencies, dict)

        generation_fields = {
            "status", "clone", "source", "command", "environment", "result",
            "reproducibility", "inventory", "raw_installer_inventory",
            "sanitized_filesystem_inventory", "safety_audit", "promotion_sanitization",
        }
        if set(generation) != generation_fields:
            abort("invalid generation proof fields")

        environment = generation.get("environment")
        if not exact_fields(
            environment, {"node", "npm", "os", "os_version", "architecture", "location"}
        ):
            abort("invalid generation environment metadata")
        for key in ("node", "npm"):
            if environment.get(key) != runtime.get(key):
                abort(f"generation environment {key} does not match locked runtime")
        for key in ("os", "os_version", "architecture"):
            if environment.get(key) != platform.get(key):
                abort(f"generation environment {key} does not match locked platform")
        if environment.get("location") != "fresh-disposable-baseline-clone":
            abort("generation environment is not a fresh disposable baseline clone")
        if generation.get("status") != "verified-reproducible-sanitized-disposable-promotion":
            abort("generation verification status is invalid")

        clone = generation.get("clone")
        if (
            not exact_fields(
                clone,
                {
                    "revision", "clean_before_each_install", "comparison_install_count",
                    "tracked_modifications_after_each_install",
                },
            )
            or clone.get("revision") != baseline.get("revision")
            or clone.get("clean_before_each_install") is not True
            or not exact_integer(clone.get("comparison_install_count"), 2)
            or clone["comparison_install_count"] < MINIMUM_REPRODUCTION_INSTALLS
            or type(clone.get("tracked_modifications_after_each_install")) is not int
            or clone["tracked_modifications_after_each_install"] != 0
        ):
            abort("generation clone proof is invalid")

        source = generation.get("source")
        if (
            not exact_fields(
                source,
                {
                    "package", "version", "dependency_payload_sha512",
                    "lifecycle_scripts_during_dependency_resolution", "package_selector",
                },
            )
            or source.get("package") != installer.get("package")
            or source.get("version") != installer.get("version")
            or source.get("dependency_payload_sha512")
            != dependencies.get("canonical_payload_sha512")
            or source.get("lifecycle_scripts_during_dependency_resolution") != "disabled"
            or source.get("package_selector") != "none-local-entry-point"
        ):
            abort("generation source identity is invalid")

        command = generation.get("command")
        command_fields = {
            "entry_point", "modules", "tools", "user_name", "communication_language",
            "document_output_language", "output_root", "project_name", "user_skill_level",
            "planning_artifacts", "implementation_artifacts", "project_knowledge",
            "non_interactive", "npm_update_lookup",
        }
        if (
            not exact_fields(command, command_fields)
            or command.get("entry_point") != installer.get("entry_point")
            or command.get("modules") != scope.get("modules")
            or command.get("tools") != scope.get("tools")
            or command.get("output_root") != scope.get("output_root")
            or command.get("planning_artifacts") != "_bmad-output/planning-artifacts"
            or command.get("implementation_artifacts")
            != "_bmad-output/implementation-artifacts"
            or command.get("project_knowledge") != "_bmad-output/project-knowledge"
            or not all(
                isinstance(command.get(field), str) and command[field].strip()
                for field in (
                    "user_name", "communication_language", "document_output_language",
                    "project_name", "user_skill_level",
                )
            )
            or command.get("non_interactive") is not True
            or command.get("npm_update_lookup") != "forced-offline"
        ):
            abort("generation command does not match locked scope")

        result = generation.get("result")
        result_fields = {
            "exit_codes", "installed_modules", "installed_tools", "launcher_roots",
            "skills_per_launcher", "files_per_launcher", "launcher_parity",
            "launcher_tree_sha256", "generated_path_rules", "human_owned_output_root",
            "unexpected_roots",
        }
        exit_codes = result.get("exit_codes") if isinstance(result, dict) else None
        if (
            not exact_fields(result, result_fields)
            or not isinstance(exit_codes, list)
            or len(exit_codes) != clone.get("comparison_install_count")
            or any(type(code) is not int or code != 0 for code in exit_codes)
            or result.get("installed_modules") != ["core", "bmm"]
            or result.get("installed_tools") != scope.get("tools")
            or result.get("launcher_roots")
            != {"codex": ".agents/skills", "claude-code": ".claude/skills"}
            or not exact_integer(result.get("skills_per_launcher"), 1)
            or not exact_integer(result.get("files_per_launcher"), 1)
            or result.get("launcher_parity") is not True
            or not isinstance(result.get("launcher_tree_sha256"), str)
            or not SHA256_RE.fullmatch(result["launcher_tree_sha256"])
            or result.get("generated_path_rules")
            != [
                "_bmad/** except _bmad/custom/config.toml and local *.user.toml",
                ".agents/skills/bmad-*/**",
                ".claude/skills/bmad-*/**",
            ]
            or result.get("human_owned_output_root") != "_bmad-output"
            or result.get("unexpected_roots") != []
        ):
            abort("generation result is inconsistent with locked scope")

        self.verify_inventory_and_promotion_proofs(generation, clone, baseline, result)
