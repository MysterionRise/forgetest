"""Locked installer, generated inventory, and manifest verification."""

from __future__ import annotations

import base64
import binascii
import os
import re
from pathlib import Path
from typing import Any

from bmad_verify_common import (
    LOCKED_VERSION,
    SHA512_BYTE_LENGTH,
    SHA256_RE,
    SHA512_RE,
    abort,
    checked_target,
    load_json_file,
    load_narrow_yaml_bytes,
    normalized_json_bytes,
    parse_version_tuple,
    path_is_within,
    sha256_bytes,
    sha256_file,
    validate_canonical_path_set,
    canonical_path,
)


class LockVerifierMixin:
    def verify_lock_paths_and_hashes(self) -> None:
        """Validate paths before reads in contract-significant fail-fast order."""
        lock_path = self.root / "bmad.lock.json"
        lock = load_json_file(lock_path, "bmad.lock.json")
        if (
            not isinstance(lock, dict)
            or type(lock.get("schema_version")) is not int
            or lock.get("schema_version") != 1
        ):
            abort("invalid BMAD lock schema")
        self.lock = lock
        self.verify_lock_contract_schema()
        self.production_shape = isinstance(lock.get("generation"), dict)
        if not self.production_shape:
            abort("BMAD lock does not use the production inventory schema")

        installer = lock.get("installer")
        if not isinstance(installer, dict):
            abort("invalid installer identity in BMAD lock")
        package = installer.get("package", installer.get("name"))
        version = installer.get("version")
        if package != "bmad-method" or version != LOCKED_VERSION:
            abort("invalid locked installer identity or locked version")
        parse_version_tuple(version, "BMAD lock")
        integrity = installer.get("integrity")
        if not isinstance(integrity, str) or not integrity.startswith("sha512-"):
            abort("missing installer integrity in BMAD lock")
        tarball_hash = installer.get("tarball_sha512")
        if not isinstance(tarball_hash, str) or not SHA512_RE.fullmatch(tarball_hash):
            abort("invalid installer SHA-512 in BMAD lock")
        try:
            integrity_bytes = base64.b64decode(integrity.removeprefix("sha512-"), validate=True)
        except (binascii.Error, ValueError):
            abort("invalid installer integrity in BMAD lock")
        if (
            len(integrity_bytes) != SHA512_BYTE_LENGTH
            or integrity_bytes.hex() != tarball_hash
        ):
            abort("installer integrity does not match locked tarball SHA-512")

        runtime = lock.get("runtime")
        if not isinstance(runtime, dict):
            abort("missing locked runtime")
        node_version = parse_version_tuple(runtime.get("node"), "runtime.node")
        parse_version_tuple(runtime.get("npm"), "runtime.npm")
        if node_version < (20, 12, 0):
            abort("locked Node runtime is below the installer minimum")

        baseline = lock.get("baseline")
        if not isinstance(baseline, dict) or not re.fullmatch(
            r"[0-9a-f]{40}", str(baseline.get("revision", ""))
        ):
            abort("invalid locked baseline revision")

        if self.production_shape:
            inventory = lock.get("generation", {}).get("inventory")
            if not isinstance(inventory, dict):
                abort("missing generated inventory in BMAD lock")
            raw_paths = inventory.get("generated_paths")
            hashes = inventory.get("generated_files_sha256")
            if not isinstance(raw_paths, list) or not isinstance(hashes, dict):
                abort("invalid generated inventory in BMAD lock")
            self.generated_paths = validate_canonical_path_set(raw_paths, "generated")
            hash_paths = validate_canonical_path_set(hashes.keys(), "generated hash")
            self.generated_hash_paths = hash_paths
            self.generated_hashes = dict(hashes)
            human_records = inventory.get("human_owned_output_roots", [])
            if not isinstance(human_records, list) or not all(
                isinstance(record, dict) for record in human_records
            ):
                abort("invalid human-owned root inventory")
            self.human_roots = validate_canonical_path_set(
                [record.get("path") for record in human_records],
                "human-owned root",
            )
        else:
            raw_files = lock.get("generated_files")
            roots = lock.get("roots")
            if not isinstance(raw_files, list) or not isinstance(roots, dict):
                abort("invalid generated inventory in BMAD lock")
            paths: list[Any] = []
            hashes: dict[str, str] = {}
            for record in raw_files:
                if not isinstance(record, dict):
                    abort("invalid generated file record")
                path = record.get("path")
                paths.append(path)
                if isinstance(path, str):
                    if path in hashes:
                        abort(f"duplicate generated file record: {path}")
                    hashes[path] = record.get("sha256")
            self.generated_paths = validate_canonical_path_set(paths, "generated")
            self.generated_hash_paths = list(self.generated_paths)
            self.generated_hashes = hashes
            generated_roots = validate_canonical_path_set(roots.get("generated", []), "generated root")
            if set(generated_roots) != {"_bmad", ".agents/skills", ".claude/skills"}:
                abort("invalid exact allowed generated roots")
            self.human_roots = validate_canonical_path_set(
                roots.get("human_owned", []), "human-owned root"
            )

        for generated in self.generated_paths:
            for human_root in self.human_roots:
                if path_is_within(generated, human_root) or path_is_within(human_root, generated):
                    abort(f"generated and human-owned path overlap: {generated}")
            if not self.generated_path_is_allowed(generated):
                abort(f"generated path is outside an approved root: {generated}")

        for relative in self.generated_paths:
            target = checked_target(self.root, relative, "generated")
            if relative in self.generated_hashes and not target.is_file():
                abort(f"missing generated file target: {relative}")

        if not set(self.generated_hash_paths).issubset(set(self.generated_paths)):
            abort("generated hash path is absent from generated path inventory")

        for relative, expected in self.generated_hashes.items():
            if not isinstance(expected, str) or not SHA256_RE.fullmatch(expected):
                abort(f"invalid generated SHA-256 for {relative}")
            actual = sha256_file(self.root / relative)
            if actual != expected:
                abort(f"generated file hash mismatch: {relative}")

        if self.production_shape:
            self.verify_full_generated_inventory()
            self.verify_human_owned_hashes()
            self.verify_dependency_payload()
            self.verify_generation_and_promotion_claims()

    def generated_path_is_allowed(self, path: str) -> bool:
        if path == "_bmad" or path.startswith("_bmad/"):
            return path != "_bmad/custom/config.toml" and not path.endswith(".user.toml")
        return bool(
            re.match(r"^\.agents/skills/bmad-[a-z0-9][a-z0-9-]*(?:/|$)", path)
            or re.match(r"^\.claude/skills/bmad-[a-z0-9][a-z0-9-]*(?:/|$)", path)
        )

    def verify_full_generated_inventory(self) -> None:
        """Validate the immutable generated tree as one ordered inventory proof."""
        inventory = self.lock["generation"]["inventory"]
        expected = set(self.generated_paths)
        discovered: set[str] = set()

        roots: list[Path] = [self.root / "_bmad"]
        for launcher_root in (self.root / ".agents" / "skills", self.root / ".claude" / "skills"):
            if not launcher_root.is_dir():
                abort(f"missing generated launcher root: {launcher_root.relative_to(self.root).as_posix()}")
            for child in sorted(launcher_root.iterdir(), key=lambda item: item.name):
                if child.name.startswith("bmad-"):
                    roots.append(child)

        for walk_root in roots:
            relative_root = walk_root.relative_to(self.root).as_posix()
            checked_target(self.root, relative_root, "generated", require_file=False)
            for current, directories, files in os.walk(walk_root, topdown=True, followlinks=False):
                current_path = Path(current)
                relative_current = current_path.relative_to(self.root).as_posix()
                if relative_current not in {
                    ".agents/skills",
                    ".claude/skills",
                }:
                    discovered.add(relative_current)
                if relative_current == "_bmad/render":
                    directories[:] = []
                    files = [name for name in files if name == ".gitignore"]
                kept_directories: list[str] = []
                for name in sorted(directories):
                    child = current_path / name
                    relative = child.relative_to(self.root).as_posix()
                    if child.is_symlink():
                        abort(f"symlink generated target is forbidden: {relative}")
                    if name == "__pycache__":
                        continue
                    kept_directories.append(name)
                directories[:] = kept_directories
                for name in sorted(files):
                    child = current_path / name
                    relative = child.relative_to(self.root).as_posix()
                    if child.is_symlink():
                        abort(f"symlink generated target is forbidden: {relative}")
                    if relative == "_bmad/custom/config.toml" or name.endswith(".user.toml"):
                        continue
                    if name.endswith((".pyc", ".pyo")):
                        continue
                    discovered.add(relative)

        if discovered != expected:
            missing = sorted(expected - discovered)
            extra = sorted(discovered - expected)
            detail = f"missing={missing[:1]} extra={extra[:1]}"
            abort(f"generated path inventory mismatch: {detail}")
        if inventory.get("path_count") != len(expected):
            abort("generated path_count does not match inventory")
        if inventory.get("file_count") != len(self.generated_hashes):
            abort("generated file_count does not match inventory")
        if inventory.get("directory_count") != len(expected) - len(self.generated_hashes):
            abort("generated directory_count does not match inventory")
        if any(inventory.get(key) for key in ("symlinks", "special_files", "absolute_paths", "traversal_paths", "duplicate_canonical_targets")):
            abort("generated safety inventory contains an unsafe path or file")

        path_payload = normalized_json_bytes(self.generated_paths)
        if sha256_bytes(path_payload) != inventory.get("path_set_sha256"):
            abort("generated path-set digest mismatch")
        tree_payload = normalized_json_bytes(
            {
                "generated_paths": self.generated_paths,
                "generated_files_sha256": self.generated_hashes,
                "symlinks": [],
            }
        )
        if sha256_bytes(tree_payload) != inventory.get("raw_promotion_sha256"):
            abort("generated inventory digest mismatch")

    def verify_human_owned_hashes(self) -> None:
        inventory = self.lock["generation"]["inventory"]
        records = inventory.get("human_owned_files", [])
        expected_fields = {"path", "sha256", "ownership", "generated_hash_excluded"}
        if (
            not isinstance(records, list)
            or len(records) != 1
            or not isinstance(records[0], dict)
            or set(records[0]) != expected_fields
            or records[0].get("path") != "_bmad/custom/config.toml"
            or records[0].get("ownership") != "human-owned-team-override"
            or records[0].get("generated_hash_excluded") is not True
        ):
            abort("invalid human-owned file inventory")
        paths = validate_canonical_path_set(
            [record.get("path") for record in records],
            "human-owned file",
        )
        for path, record in zip(paths, records):
            target = checked_target(self.root, path, "human-owned", require_file=True)
            expected = record.get("sha256")
            if not isinstance(expected, str) or not SHA256_RE.fullmatch(expected):
                abort(f"invalid human-owned SHA-256 for {path}")
            if sha256_file(target) != expected:
                abort(f"human-owned file hash mismatch: {path}")

        roots = inventory.get("human_owned_output_roots")
        if (
            not isinstance(roots, list)
            or len(roots) != 1
            or roots[0]
            != {
                "path": "_bmad-output",
                "state_after_install": "empty",
                "generated_path_excluded": True,
            }
        ):
            abort("invalid human-owned root inventory")
        if records[0]["path"] in self.generated_paths:
            abort("human-owned team override is present in generated inventory")

    def verify_manifest_and_locked_scope(self) -> None:
        manifest_relative = self.lock.get("manifest", "_bmad/_config/manifest.yaml")
        manifest_relative = canonical_path(manifest_relative, "manifest")
        if manifest_relative not in self.generated_hashes:
            abort("manifest is absent from generated hash inventory")
        manifest_path = checked_target(self.root, manifest_relative, "manifest", require_file=True)
        manifest = load_narrow_yaml_bytes(manifest_path.read_bytes(), manifest_relative)
        if not isinstance(manifest, dict):
            abort("invalid manifest mapping")
        self.manifest = manifest

        if "installation" in manifest:
            installation = manifest.get("installation")
            modules = manifest.get("modules")
            tools = manifest.get("ides")
            if not isinstance(installation, dict) or installation.get("version") != LOCKED_VERSION:
                abort("manifest installation version does not match locked version")
            if not isinstance(modules, list) or not all(isinstance(item, dict) for item in modules):
                abort("invalid manifest module inventory")
            names = [item.get("name") for item in modules]
            versions = [item.get("version") for item in modules]
            if names != ["core", "bmm"] or versions != [LOCKED_VERSION, LOCKED_VERSION]:
                abort("manifest module scope does not match locked BMM-only installation")
            if tools != ["codex", "claude-code"]:
                abort("manifest tool scope does not match lock")
            scope = self.lock.get("scope")
            if not isinstance(scope, dict) or scope.get("modules") != ["bmm"] or scope.get("tools") != tools:
                abort("manifest and lock scope mismatch")
        else:
            if manifest.get("module") != "bmm":
                abort("manifest module does not match lock")
            if manifest.get("module_version") != LOCKED_VERSION:
                abort("manifest module version does not match locked version")
            enabled = self.lock.get("enabled")
            if not isinstance(enabled, dict):
                abort("invalid enabled scope in BMAD lock")
            modules = enabled.get("modules")
            if not isinstance(modules, list) or len(modules) != 1 or not isinstance(modules[0], dict):
                abort("invalid locked module scope")
            if modules[0].get("id") != "bmm" or modules[0].get("version") != LOCKED_VERSION:
                abort("manifest and lock module mismatch")
            if enabled.get("tools") != ["codex", "claude-code"] or manifest.get("tools") != enabled.get("tools"):
                abort("manifest and lock tool mismatch")
