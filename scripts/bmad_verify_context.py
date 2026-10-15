"""Authoritative source, additive CI, and publication-boundary checks."""

from __future__ import annotations

import os
import re
from pathlib import Path
from urllib.parse import unquote_to_bytes

from bmad_verify_common import (
    CANONICAL_CI_GATE,
    CI_GATE_ANCHOR,
    END_GATE,
    SHA256_RE,
    START_GATE,
    abort,
    canonical_path,
    checked_target,
    exact_structure,
    parse_front_matter,
    sha256_bytes,
    sha256_file,
    timestamp_is_valid,
    validate_canonical_path_set,
)


APPROVED_CONTEXT_REVIEWED_AT = "2026-08-29T12:15:33Z"
APPROVED_CONTEXT_BASIS = (
    "Don-approved 2026-08-29 BMAD repository setup specification review"
)
ONBOARDING_RUNTIME_RE = re.compile(
    r"BMAD launcher rendering requires `uv`\s+and follows the renderer's PEP 723\s+"
    r"`requires-python = \">=3\.11\"` declaration\. The repository BMAD verifier is\s+"
    r"separate and requires Python >=3\.10\."
)
PUBLICATION_SCRIPT_RE = re.compile(
    rb"(?:\$\{?ROOT\}?/|\./)?(scripts/[A-Za-z0-9._/-]+\.sh)\b"
)
SCRIPT_DIR_PUBLICATION_RE = re.compile(
    rb"\$\{?SCRIPT_DIR\}?/([A-Za-z0-9._/-]+\.sh)\b"
)


class ContextVerifierMixin:
    def verify_context_identities(self) -> None:
        """Validate the reviewed context identities as one ordered proof chain."""
        context_path = checked_target(
            self.root, "_bmad-output/project-context.md", "project context", require_file=True
        )
        context = parse_front_matter(context_path, "project context")
        self.context = context
        identities = context.get("source_identities")
        if not isinstance(identities, dict) or not identities:
            abort("project context source identity inventory is missing")
        source_paths = validate_canonical_path_set(identities.keys(), "context source")
        for relative in source_paths:
            expected = identities[relative]
            if not isinstance(expected, str) or not SHA256_RE.fullmatch(expected):
                abort(f"invalid project context source identity: {relative}")
            target = checked_target(self.root, relative, "context source", require_file=True)
            if sha256_file(target) != expected:
                abort(f"project context drift detected: {relative}")
        if self.production_shape:
            required = {
                "Cargo.toml", "ARCHITECTURE.md", "SECURITY.md",
                "studies/rust-agent-v1/README.md", "studies/rust-agent-pilot/README.md",
                "docs/book.toml", "docs/src/SUMMARY.md", ".github/workflows/ci.yml",
            }
            if set(source_paths) != required:
                abort("project context authoritative source inventory is incomplete")
            if context.get("locked_baseline_revision") != self.lock["baseline"]["revision"]:
                abort("project context baseline revision does not match lock")
            review = context.get("review")
            if not isinstance(review, dict):
                abort("project context review metadata is missing")
            reviewer = self.authorities.get(review.get("reviewer_identity"))
            required_review_fields = {
                "reviewed_at",
                "reviewer_identity",
                "reviewer_identity_type",
                "reviewer_role",
                "approval_basis",
            }
            if not required_review_fields.issubset(review):
                abort("project context review metadata is incomplete")
            if not timestamp_is_valid(review.get("reviewed_at")):
                abort("project context review timestamp is invalid")
            if (
                not reviewer
                or reviewer.get("status") != "active"
                or reviewer.get("verified_human") is not True
                or reviewer.get("role") != review.get("reviewer_role")
                or reviewer.get("identity_type") != review.get("reviewer_identity_type")
                or not self.authority_is_eligible(review.get("reviewer_identity"), "repository")
            ):
                abort("project context reviewer identity is ineligible")
            if not isinstance(review.get("approval_basis"), str) or not review["approval_basis"].strip():
                abort("project context review approval basis is missing")
            if (
                review.get("reviewed_at") != APPROVED_CONTEXT_REVIEWED_AT
                or review.get("approval_basis") != APPROVED_CONTEXT_BASIS
            ):
                abort("project context approved review metadata is stale")
            self.verify_onboarding_contract(context_path)

    def verify_onboarding_contract(self, context_path: Path) -> None:
        readme_path = checked_target(
            self.root, "README.md", "README BMAD runtime onboarding", require_file=True
        )
        try:
            readme = readme_path.read_text(encoding="utf-8")
            context_text = context_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            abort("unable to inspect BMAD runtime onboarding")
        if ONBOARDING_RUNTIME_RE.search(readme) is None:
            abort("README BMAD runtime onboarding is incomplete")
        if ONBOARDING_RUNTIME_RE.search(context_text) is None:
            abort("project context BMAD runtime onboarding is incomplete")

        smoke_path = checked_target(
            self.root,
            "scripts/test-bmad-render.sh",
            "BMAD launcher render smoke check",
            require_file=True,
        )
        try:
            smoke = smoke_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            abort("unable to inspect BMAD launcher render smoke check")
        required = (
            "UV_PYTHON_DOWNLOADS=never UV_OFFLINE=1",
            "uv run --no-cache",
            '"$ROOT/_bmad/scripts/render_skill.py"',
            '"$ROOT/.agents/skills/bmad-build"',
        )
        if any(token not in smoke for token in required):
            abort("BMAD launcher render smoke contract is incomplete")

    def baseline_ci_bytes(self, data: bytes) -> tuple[bytes, bytes | None]:
        start_count = data.count(START_GATE)
        end_count = data.count(END_GATE)
        if start_count == 0 and end_count == 0:
            return data, None
        if start_count != 1 or end_count != 1:
            abort("CI additive BMAD gate markers are invalid")
        start = data.find(START_GATE)
        end = data.find(END_GATE)
        if end < start:
            abort("CI additive BMAD gate marker order is invalid")
        block_end = end + len(END_GATE)
        return data[:start] + data[block_end:], data[start:block_end]

    def validate_ci_gate(self, gate: bytes) -> None:
        if gate != CANONICAL_CI_GATE:
            abort("CI additive BMAD governance gate does not match the approved canonical job")

    def verify_ci_baseline(self) -> None:
        baseline = self.lock.get("baseline")
        if not isinstance(baseline, dict):
            abort("invalid CI baseline lock")
        ci_metadata = baseline.get("ci")
        if isinstance(ci_metadata, dict):
            ci_path_raw = ci_metadata.get("workflow")
            expected_hash = ci_metadata.get("workflow_sha256")
        else:
            ci_path_raw = baseline.get("ci_path")
            expected_hash = baseline.get("ci_sha256")
        ci_path = canonical_path(ci_path_raw, "CI")
        target = checked_target(self.root, ci_path, "CI", require_file=True)
        current = target.read_bytes()
        reconstructed, gate = self.baseline_ci_bytes(current)
        if not isinstance(expected_hash, str) or not SHA256_RE.fullmatch(expected_hash):
            abort("invalid CI baseline SHA-256")
        if sha256_bytes(reconstructed) != expected_hash:
            abort("CI baseline drift detected after removing the additive BMAD gate")
        if self.production_shape:
            if gate is None:
                abort("CI additive BMAD governance gate is missing")
            self.validate_ci_gate(gate)
            anchor_count = reconstructed.count(CI_GATE_ANCHOR)
            expected_offset = reconstructed.find(CI_GATE_ANCHOR) + len(CI_GATE_ANCHOR)
            actual_offset = current.find(START_GATE)
            if anchor_count != 1 or actual_offset != expected_offset:
                abort("CI additive BMAD governance gate is not at the approved quality offset")
            if not isinstance(ci_metadata, dict):
                abort("invalid CI baseline inventory")
            jobs = ci_metadata.get("jobs")
            if (
                not isinstance(jobs, list)
                or type(ci_metadata.get("job_count")) is not int
                or ci_metadata.get("job_count") != len(jobs)
            ):
                abort("CI baseline job inventory is invalid")
            normalized_jobs: list[dict[str, object]] = []
            step_count = 0
            for job in jobs:
                if (
                    not isinstance(job, dict)
                    or not isinstance(job.get("id"), str)
                    or not isinstance(job.get("name"), str)
                    or not isinstance(job.get("steps"), list)
                ):
                    abort("CI baseline job inventory is invalid")
                step_names: list[str] = []
                for step in job["steps"]:
                    if not isinstance(step, dict) or not isinstance(step.get("name"), str):
                        abort("CI baseline step inventory is invalid")
                    step_names.append(step["name"])
                step_count += len(step_names)
                normalized_jobs.append(
                    {"id": job["id"], "name": job["name"], "steps": step_names}
                )
            if (
                type(ci_metadata.get("step_count")) is not int
                or ci_metadata.get("step_count") != step_count
            ):
                abort("CI baseline step inventory is invalid")
            context_contract = self.context.get("ci_proof_contract")
            if not isinstance(context_contract, dict):
                abort("project context CI proof contract is missing")
            expected_contract = {
                "workflow": ci_metadata.get("workflow"),
                "job_count": ci_metadata.get("job_count"),
                "step_count": ci_metadata.get("step_count"),
                "jobs": normalized_jobs,
            }
            if not exact_structure(context_contract, expected_contract):
                abort("project context CI proof contract diverges from lock")

    def verify_publication_boundary(self) -> None:
        """Validate source and rendered publication surfaces in one deny-first scan."""
        book_path = checked_target(self.root, "docs/book.toml", "publication config", require_file=True)
        text = book_path.read_text(encoding="utf-8")
        source_relative = canonical_path(
            f"docs/{self.book_source(text)}", "publication source"
        )
        if source_relative != "docs/src":
            abort("BMAD publication source root is forbidden")
        source_root = checked_target(self.root, source_relative, "publication source", require_file=False)
        for current, directories, files in os.walk(source_root, topdown=True, followlinks=False):
            current_path = Path(current)
            for name in list(directories) + list(files):
                candidate = current_path / name
                if candidate.is_symlink():
                    abort(f"symlink publication target is forbidden: {candidate.relative_to(self.root).as_posix()}")
            for name in files:
                path = current_path / name
                try:
                    data = path.read_bytes()
                except OSError:
                    relative = path.relative_to(self.root).as_posix()
                    abort(f"unable to inspect publication path: {relative}")
                if self.contains_internal_publication_path(data):
                    abort(f"internal BMAD publication reference detected: {path.relative_to(self.root).as_posix()}")
        self.verify_publication_build_surfaces()

        rendered_roots: list[str] = []
        required_rendered = os.environ.get("BMAD_RENDERED_OUTPUT")
        if required_rendered:
            relative = self.canonical_rendered_output(required_rendered)
            candidate = self.root / relative
            if not candidate.exists() and not candidate.is_symlink():
                abort(f"required rendered publication output is missing: {relative}")
            checked_target(
                self.root, relative, "rendered publication", require_file=False
            )
            rendered_roots.append(relative)
        for relative in ("target/book", "target/pages", "docs/book", "book"):
            if (self.root / relative).is_dir() and relative not in rendered_roots:
                rendered_roots.append(relative)
        for rendered_relative in rendered_roots:
            rendered = checked_target(
                self.root,
                rendered_relative,
                "rendered publication",
                require_file=False,
            )
            for path in sorted(rendered.rglob("*")):
                repository_relative = path.relative_to(self.root).as_posix()
                if path.is_symlink():
                    abort(
                        "symlink rendered publication target is forbidden: "
                        f"{repository_relative}"
                    )
                relative = path.relative_to(rendered).as_posix().lower()
                if self.contains_internal_publication_path(relative.encode()):
                    abort(f"internal BMAD rendered publication detected: {rendered_relative}")
                if path.is_file():
                    try:
                        data = path.read_bytes()
                    except OSError:
                        abort(f"unable to inspect rendered publication: {repository_relative}")
                    if self.contains_internal_publication_path(data):
                        abort(f"internal BMAD rendered publication content detected: {relative}")

    def canonical_rendered_output(self, raw: str) -> str:
        invalid = (
            not raw
            or "\x00" in raw
            or raw.startswith("/")
            or "\\" in raw
            or re.match(r"^[A-Za-z]:[\\/]", raw) is not None
            or ".." in raw.split("/")
        )
        if invalid:
            abort("rendered publication path is not a canonical repository-relative directory")
        relative = canonical_path(raw, "rendered publication")
        if raw != relative or "//" in raw or "/./" in f"/{raw}/":
            abort("non-canonical rendered publication path is forbidden")
        return relative

    def book_source(self, text: str) -> str:
        section: str | None = None
        book_count = 0
        sources: list[str] = []
        for raw_line in text.splitlines():
            line = raw_line.strip()
            header = re.fullmatch(r"\[([^\]]+)\]", line)
            if header:
                section = header.group(1).strip()
                if section == "book":
                    book_count += 1
                continue
            if section == "book":
                match = re.fullmatch(r'src\s*=\s*["\']([^"\']+)["\']\s*(?:#.*)?', line)
                if match:
                    sources.append(match.group(1))
        if book_count != 1 or len(sources) != 1:
            abort("publication source root is missing or ambiguous in docs/book.toml [book]")
        return sources[0]

    def contains_internal_publication_path(self, data: bytes) -> bool:
        decoded = data
        for _ in range(3):
            lowered = decoded.lower()
            if b"_bmad" in lowered or b"_bmad-output" in lowered:
                return True
            next_value = unquote_to_bytes(decoded)
            if next_value == decoded:
                break
            decoded = next_value
        lowered = decoded.lower()
        return b"_bmad" in lowered or b"_bmad-output" in lowered

    def verify_publication_build_surfaces(self) -> None:
        script_queue = list((self.root / "scripts").glob("*pages*.sh"))
        candidates: set[Path] = set()
        workflows = self.root / ".github" / "workflows"
        if workflows.is_dir():
            for path in workflows.iterdir():
                if path.suffix.lower() not in {".yml", ".yaml"} or not path.is_file():
                    continue
                data = path.read_bytes()
                lowered = data.lower()
                if any(
                    marker in lowered
                    for marker in (b"pages", b"mdbook", b"build-pages", b"upload-pages")
                ):
                    candidates.add(path)
                    all_references = any(
                        marker in path.name.lower() for marker in ("pages", "mdbook")
                    )
                    for relative in self.publication_script_references(data, path):
                        name = Path(relative).name.lower()
                        if all_references or "pages" in name or "mdbook" in name:
                            script_queue.append(self.root / relative)
        seen_scripts: set[Path] = set()
        while script_queue:
            candidate = script_queue.pop()
            try:
                relative = candidate.relative_to(self.root).as_posix()
            except ValueError:
                abort("publication build helper path escapes repository")
            canonical = canonical_path(relative, "publication build helper")
            if canonical != relative:
                abort(f"non-canonical publication build helper path is forbidden: {relative}")
            path = checked_target(
                self.root, canonical, "publication build helper", require_file=True
            )
            if path in seen_scripts:
                continue
            seen_scripts.add(path)
            candidates.add(path)
            data = path.read_bytes()
            for referenced in self.publication_script_references(data, path):
                script_queue.append(self.root / referenced)
        for path in sorted(candidates):
            if path.is_symlink():
                relative = path.relative_to(self.root).as_posix()
                abort(f"symlink publication build surface is forbidden: {relative}")
            try:
                data = path.read_bytes()
            except OSError:
                relative = path.relative_to(self.root).as_posix()
                abort(f"unable to inspect publication build surface: {relative}")
            if self.contains_internal_publication_path(data):
                relative = path.relative_to(self.root).as_posix()
                abort(f"internal BMAD publication copy or workflow rule detected: {relative}")

    def publication_script_references(self, data: bytes, source: Path) -> set[str]:
        decoded = data
        for _ in range(3):
            next_value = unquote_to_bytes(decoded)
            if next_value == decoded:
                break
            decoded = next_value
        raw_references = {
            match.group(1).decode("ascii") for match in PUBLICATION_SCRIPT_RE.finditer(decoded)
        }
        source_parent = source.relative_to(self.root).parent.as_posix()
        raw_references.update(
            f"{source_parent}/{match.group(1).decode('ascii')}"
            for match in SCRIPT_DIR_PUBLICATION_RE.finditer(decoded)
        )
        references: set[str] = set()
        for raw in raw_references:
            canonical = canonical_path(raw, "publication build helper")
            if raw != canonical:
                abort(f"non-canonical publication build helper path is forbidden: {raw}")
            references.add(canonical)
        return references
