"""Protected-study discovery, full-tree identity, and instruction guards."""

from __future__ import annotations

import re
from pathlib import Path

from bmad_verify_common import (
    POLICY_END,
    POLICY_START,
    SHA256_RE,
    abort,
    exact_structure,
    normalized_json_bytes,
    sha256_bytes,
    sha256_file,
    validate_canonical_path_set,
)


PROTOCOL_NAMES = {
    "protocol.md", "protocol.toml", "protocol.yaml", "protocol.yml", "protocol.json"
}
PROTECTED_MARKER = re.compile(r"\b(?:frozen|locked|pre-registered)\b", re.I)
MUTATION_VERBS = (
    "modify", "mutate", "edit", "write", "changes?", "scratch", "alter", "overwrite",
    "patch", "replace", "delete", "remove", "rewrite", "regenerate", "touch", "update",
)
MUTATION_RE = re.compile(rf"\b(?:{'|'.join(MUTATION_VERBS)})\b", re.I)
DIRECT_NEGATION_RE = re.compile(
    rf"\b(?:never|must\s+not|do\s+not|cannot|can\s+not|can't|mustn't)"
    rf"(?:\s+[a-z-]+){{0,2}}\s+(?:{'|'.join(MUTATION_VERBS)})\b",
    re.I,
)


class StudyVerifierMixin:
    def protected_study_roots(self) -> list[str]:
        studies = self.root / "studies"
        protected: set[str] = set()
        for fixed in ("studies/rust-agent-v1", "studies/rust-agent-pilot"):
            if (self.root / fixed).is_dir():
                protected.add(fixed)
        if studies.is_dir():
            for declaration in self.protocol_declarations(studies):
                relative = declaration.relative_to(self.root).as_posix()
                try:
                    text = declaration.read_text(encoding="utf-8")
                except (OSError, UnicodeError):
                    abort(
                        "unable to read protected study protocol declaration: "
                        f"{relative}"
                    )
                if PROTECTED_MARKER.search(text):
                    protected.add(declaration.parent.relative_to(self.root).as_posix())
        return sorted(protected)

    def protocol_declarations(self, studies: Path) -> list[Path]:
        declarations: list[Path] = []
        for path in sorted(studies.rglob("*")):
            if path.is_symlink():
                abort(
                    "symlink protected study protocol is forbidden: "
                    f"{path.relative_to(self.root).as_posix()}"
                )
            if not path.is_file():
                continue
            name = path.name.lower()
            root_readme = name == "readme.md" and path.parent.parent == studies
            conventional = name in PROTOCOL_NAMES or (
                path.suffix.lower() in {".md", ".toml", ".yaml", ".yml", ".json"}
                and re.search(r"(?:^|[-_.])protocol(?:[-_.]|$)", name) is not None
            )
            if root_readme or conventional:
                declarations.append(path)
        return declarations

    def verify_study_guards(self) -> None:
        protected = self.protected_study_roots()
        self.verify_protected_study_trees(protected)
        for path in self.artifact_text_paths():
            relative = path.relative_to(self.root).as_posix()
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError):
                abort(
                    "unable to read shared artifact instruction surface: "
                    f"{relative}"
                )
            if POLICY_START in text and POLICY_END in text:
                start = text.index(POLICY_START)
                end = text.index(POLICY_END, start) + len(POLICY_END)
                text = text[:start] + text[end:]
            sentences = re.split(r"(?<=[.!?])\s+|\n\s*\n", text)
            for sentence in sentences:
                if not MUTATION_RE.search(sentence):
                    continue
                for study in protected:
                    if study in sentence and not self.study_instruction_is_safe(sentence):
                        abort(
                            "protected study modification instruction in "
                            f"{relative}"
                        )

    def study_instruction_is_safe(self, line: str) -> bool:
        return bool(
            DIRECT_NEGATION_RE.search(line)
            or re.search(r"\b(?:is|are|remains?|stay)\s+read-only\b", line, re.I)
            or re.search(r"\brequires?\s+(?:a\s+)?new\s+study\b", line, re.I)
            or re.search(r"\bno\s+exemption\b", line, re.I)
        )

    def verify_protected_study_trees(self, protected: list[str]) -> None:
        identities = self.context.get("protected_study_tree_identities")
        if not isinstance(identities, dict) or identities.get(
            "algorithm"
        ) != "sha256-normalized-file-manifest-v1":
            abort("project context protected study tree identities are missing")
        records = identities.get("roots")
        if not isinstance(records, dict):
            abort("project context protected study tree identities are missing")
        registered = validate_canonical_path_set(records.keys(), "protected study tree")
        missing = sorted(set(protected) - set(registered))
        if missing:
            abort(f"protected study tree identity is missing: {missing[0]}")
        if set(registered) != set(protected):
            abort("project context protected study tree inventory is stale")
        for relative in registered:
            record = records[relative]
            if (
                not isinstance(record, dict)
                or set(record) != {"file_count", "tree_sha256"}
                or type(record.get("file_count")) is not int
                or record["file_count"] < 1
                or not isinstance(record.get("tree_sha256"), str)
                or SHA256_RE.fullmatch(record["tree_sha256"]) is None
            ):
                abort(f"invalid protected study tree identity: {relative}")
            manifest = self.study_tree_manifest(relative)
            actual = sha256_bytes(normalized_json_bytes(manifest))
            expected = {"file_count": len(manifest), "tree_sha256": actual}
            if not exact_structure(record, expected):
                abort(f"protected study tree drift detected: {relative}")

    def study_tree_manifest(self, relative: str) -> dict[str, str]:
        root = self.root / relative
        manifest: dict[str, str] = {}
        for path in sorted(root.rglob("*")):
            if path.is_symlink():
                abort(
                    "symlink protected study path is forbidden: "
                    f"{path.relative_to(self.root).as_posix()}"
                )
            if path.is_file():
                manifest[path.relative_to(root).as_posix()] = sha256_file(path)
        return manifest
