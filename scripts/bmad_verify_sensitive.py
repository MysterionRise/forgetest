"""Sensitive-content coverage for committed BMAD and governance surfaces."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path
from typing import Any

from bmad_verify_common import abort, load_json_file


SENSITIVE_PATTERNS = (
    re.compile(rb"SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"),
    re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    re.compile(rb"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(rb"\bgh[oprsu]_[A-Za-z0-9]{30,}\b"),
    re.compile(rb"\bsk-[A-Za-z0-9_-]{24,}\b"),
    re.compile(
        rb"(?i)\b(?:credentials|secrets|personal-data|"
        rb"customer-confidential(?:-information)?|raw-private-evidence|"
        rb"live-exploit-material|restricted-storage-material)\s*:\s*(?:true|yes)\b"
    ),
)


class SensitiveVerifierMixin:
    def verify_sensitive_content(self) -> None:
        committed = self.committed_bmad_blobs()
        candidates = {self.root / relative for relative in self.generated_hashes}
        candidates.update(self.artifact_text_paths())
        candidates.update(committed)
        candidates.update(self.canonical_artifact_files())
        candidates.update(self.approval_evidence_files())
        candidates.update(self.bootstrap_evidence_files())
        for relative in (
            "bmad.lock.json",
            "_bmad/custom/config.toml",
            "_bmad-output/authorities.yaml",
            "_bmad-output/index.yaml",
            "README.md",
        ):
            path = self.root / relative
            if path.is_file():
                candidates.add(path)

        for path in sorted(candidates):
            relative = path.relative_to(self.root).as_posix()
            payloads = list(committed.get(path, []))
            if path not in committed or (path.exists() and not path.is_symlink()):
                try:
                    payloads.append(path.read_bytes())
                except OSError:
                    abort(f"unable to scan sensitive content in {relative}")
            if not payloads:
                abort(f"unable to scan sensitive content in {relative}")
            if any(pattern.search(data) for data in payloads for pattern in SENSITIVE_PATTERNS):
                abort(f"sensitive content detected in {relative}")

    def committed_bmad_blobs(self) -> dict[Path, list[bytes]]:
        try:
            listing = subprocess.run(
                [
                    "git", "-C", str(self.root), "ls-files", "-s", "-z", "--",
                    ".agents", ".claude", "_bmad", "_bmad-output",
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except (OSError, subprocess.CalledProcessError):
            abort("unable to enumerate committed BMAD files for sensitive-content scan")

        entries: list[tuple[Path, bytes]] = []
        for raw in listing.stdout.split(b"\0"):
            if not raw:
                continue
            try:
                metadata, raw_path = raw.split(b"\t", 1)
                _mode, object_id, _stage = metadata.split()
                relative = raw_path.decode("utf-8")
            except (UnicodeDecodeError, ValueError):
                abort("invalid repository path in sensitive-content scan")
            entries.append((self.root / relative, object_id))
        if not entries:
            return {}

        try:
            batch = subprocess.run(
                ["git", "-C", str(self.root), "cat-file", "--batch"],
                input=b"".join(object_id + b"\n" for _, object_id in entries),
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            ).stdout
        except (OSError, subprocess.CalledProcessError):
            abort("unable to read committed BMAD blobs for sensitive-content scan")

        blobs: dict[Path, list[bytes]] = {}
        offset = 0
        for path, object_id in entries:
            line_end = batch.find(b"\n", offset)
            if line_end < 0:
                abort("invalid committed BMAD blob response")
            header = batch[offset:line_end].split()
            try:
                size = int(header[2])
            except (IndexError, ValueError):
                abort("invalid committed BMAD blob response")
            data_start = line_end + 1
            data_end = data_start + size
            if header[:2] != [object_id, b"blob"] or batch[data_end:data_end + 1] != b"\n":
                abort("invalid committed BMAD blob response")
            blobs.setdefault(path, []).append(batch[data_start:data_end])
            offset = data_end + 1
        if offset != len(batch):
            abort("invalid committed BMAD blob response")
        return blobs

    def canonical_artifact_files(self) -> set[Path]:
        paths: set[Path] = set()
        for relative in self.index.get("canonical_roots", []):
            root = self.root / str(relative)
            if root.is_dir():
                paths.update(path for path in root.rglob("*") if path.is_file())
        return paths

    def approval_evidence_files(self) -> set[Path]:
        paths: set[Path] = set()
        registry = load_json_file(
            self.root / "_bmad-output/authorities.yaml", "authority registry"
        )
        if isinstance(registry, dict):
            approval = registry.get("registry_approval")
            if isinstance(approval, dict):
                self.add_evidence_path(paths, approval.get("evidence"))
        for record in self.artifacts:
            for field in (
                "approval_evidence",
                "independence_exception_evidence",
                "supersession_evidence",
                "archive_evidence",
            ):
                self.add_evidence_path(paths, record.get(field))
        return paths

    def add_evidence_path(self, paths: set[Path], raw: Any) -> None:
        if isinstance(raw, str):
            path = self.root / raw
            if path.is_file():
                paths.add(path)

    def bootstrap_evidence_files(self) -> set[Path]:
        root = self.root / "docs/gangsta/bmad-repository-setup"
        if not root.is_dir():
            return set()
        return {path for path in root.rglob("*") if path.is_file()}
