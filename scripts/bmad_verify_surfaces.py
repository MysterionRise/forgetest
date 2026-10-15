"""Launcher, instruction, local-state, study, and sensitive-data checks."""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

from bmad_verify_common import (
    abort,
    sha256_file,
)


class SurfaceVerifierMixin:
    def verify_instruction_and_launcher_parity(self) -> None:
        """Validate both launcher surfaces as one ordered parity proof."""
        agents_root = self.root / ".agents" / "skills"
        claude_root = self.root / ".claude" / "skills"
        if not agents_root.is_dir() or not claude_root.is_dir():
            abort("missing launcher roots")
        agent_files = self.launcher_file_map(agents_root)
        claude_files = self.launcher_file_map(claude_root)
        if set(agent_files) != set(claude_files):
            abort("launcher workflow identity inventory diverges")
        byte_mismatches = [
            relative
            for relative in sorted(agent_files)
            if agent_files[relative] != claude_files[relative]
        ]

        if byte_mismatches:
            abort(f"launcher workflow identity bytes diverge: {byte_mismatches[0]}")

        self.verify_skill_manifest(agent_files)
        self.verify_repository_instructions()

    def launcher_file_map(self, root: Path) -> dict[str, str]:
        result: dict[str, str] = {}
        for current, directories, files in os.walk(root, topdown=True, followlinks=False):
            current_path = Path(current)
            directories[:] = sorted(name for name in directories if name.startswith("bmad-") or current_path != root)
            for name in sorted(files):
                path = current_path / name
                if path.is_symlink():
                    abort(f"symlink launcher path is forbidden: {path.relative_to(self.root).as_posix()}")
                relative = path.relative_to(root).as_posix()
                if not relative.split("/", 1)[0].startswith("bmad-"):
                    continue
                result[relative] = sha256_file(path)
        return result

    def verify_tracked_local_state(self) -> None:
        """Validate tracked and visible local state in one fail-fast scan."""
        try:
            result = subprocess.run(
                [
                    "git",
                    "-C",
                    str(self.root),
                    "ls-files",
                    "-z",
                    "--",
                    ".agents",
                    ".claude",
                    ".codex",
                    "_bmad",
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except (OSError, subprocess.CalledProcessError) as exc:
            abort(f"unable to inspect tracked Claude and Codex state: {exc}")
        tracked = [entry.decode("utf-8") for entry in result.stdout.split(b"\0") if entry]
        for path in tracked:
            transient = (
                "/__pycache__/" in f"/{path}"
                or path.endswith("/__pycache__")
                or path.endswith((".pyc", ".pyo"))
            )
            allowed_agent = bool(
                re.match(r"^\.agents/skills/bmad-[a-z0-9][a-z0-9-]*/", path)
            ) and not transient
            allowed_claude = bool(
                re.match(r"^\.claude/skills/bmad-[a-z0-9][a-z0-9-]*/", path)
            ) and not transient
            allowed_bmad = (
                path.startswith("_bmad/")
                and not path.endswith(".user.toml")
                and not transient
                and not (path.startswith("_bmad/render/") and path != "_bmad/render/.gitignore")
            )
            if path.startswith(".agents/") and not allowed_agent:
                abort(f"disallowed tracked Codex state: {path}")
            if path.startswith(".claude/") and not allowed_claude:
                abort(f"disallowed tracked Claude state: {path}")
            if path.startswith(".codex/"):
                abort(f"disallowed tracked Codex state: {path}")
            if path.startswith("_bmad/") and not allowed_bmad:
                abort(f"disallowed tracked BMAD local state: {path}")

        if self.production_shape:
            try:
                result = subprocess.run(
                    [
                        "git", "-C", str(self.root), "ls-files", "-z",
                        "--others", "--exclude-standard", "--",
                        ".agents", ".claude", ".codex", "_bmad",
                    ],
                    check=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
            except (OSError, subprocess.CalledProcessError) as exc:
                abort(f"unable to inspect untracked Claude, Codex, and BMAD state: {exc}")
            visible_untracked = {
                entry.decode("utf-8") for entry in result.stdout.split(b"\0") if entry
            }
            visible_user_state = sorted(
                path
                for path in visible_untracked
                if path.startswith("_bmad/") and path.endswith(".user.toml")
            )
            if visible_user_state:
                abort(
                    "local Claude, Codex, or BMAD state is not ignored: "
                    f"{visible_user_state[0]}"
                )
            probes = (
                ".agents/settings.local.json", ".claude/settings.local.json",
                ".codex/settings.json", "_bmad/config.user.toml",
                "_bmad/custom/example.user.toml",
                "_bmad/workflows/local.user.toml",
                "_bmad/workflows/__pycache__/local.pyc",
            )
            for probe in probes:
                result = subprocess.run(
                    ["git", "-C", str(self.root), "check-ignore", "--no-index", "-q", probe],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                if result.returncode != 0 or probe in visible_untracked:
                    abort(f"local Claude, Codex, or BMAD state is not ignored: {probe}")

    def artifact_text_paths(self) -> list[Path]:
        paths = [self.root / str(record["path"]) for record in self.artifacts]
        for relative in (
            "README.md",
            "AGENTS.md",
            "CLAUDE.md",
            "_bmad-output/project-context.md",
            "_bmad-output/planning-artifacts/README.md",
            "_bmad-output/implementation-artifacts/README.md",
        ):
            path = self.root / relative
            if path.is_file():
                paths.append(path)
        return paths
