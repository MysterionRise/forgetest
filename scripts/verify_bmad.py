#!/usr/bin/env python3
"""Deterministic, standard-library-only verification for repository BMAD state."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any, Sequence

from bmad_verify_authority import AuthorityVerifierMixin
from bmad_verify_common import VerificationError, retrospective_deadline
from bmad_verify_context import ContextVerifierMixin
from bmad_verify_governance import GovernanceVerifierMixin
from bmad_verify_lock import LockVerifierMixin
from bmad_verify_policy import PolicyVerifierMixin
from bmad_verify_promotion import PromotionVerifierMixin
from bmad_verify_provenance import ProvenanceVerifierMixin
from bmad_verify_sensitive import SensitiveVerifierMixin
from bmad_verify_studies import StudyVerifierMixin
from bmad_verify_surfaces import SurfaceVerifierMixin

__all__ = ["Verifier", "main", "retrospective_deadline"]


class Verifier(
    AuthorityVerifierMixin,
    LockVerifierMixin,
    ProvenanceVerifierMixin,
    PromotionVerifierMixin,
    PolicyVerifierMixin,
    StudyVerifierMixin,
    GovernanceVerifierMixin,
    SurfaceVerifierMixin,
    SensitiveVerifierMixin,
    ContextVerifierMixin,
):
    def __init__(self, root: Path) -> None:
        self.root = root
        self.lock: dict[str, Any] = {}
        self.production_shape = False
        self.generated_paths: list[str] = []
        self.generated_hashes: dict[str, str] = {}
        self.generated_hash_paths: list[str] = []
        self.human_roots: list[str] = []
        self.manifest: dict[str, Any] = {}
        self.authorities: dict[str, dict[str, Any]] = {}
        self.index: dict[str, Any] = {}
        self.artifacts: list[dict[str, Any]] = []
        self.context: dict[str, Any] = {}

    def run(self) -> None:
        self.verify_lock_paths_and_hashes()
        self.verify_manifest_and_locked_scope()
        self.verify_authorities_and_artifacts()
        self.verify_instruction_and_launcher_parity()
        self.verify_tracked_local_state()
        self.verify_context_identities()
        self.verify_study_guards()
        self.verify_sensitive_content()
        self.verify_ci_baseline()
        self.verify_publication_boundary()


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="scripts/verify-bmad.sh",
        description="Verify the repository BMAD installation and governance contracts.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help=argparse.SUPPRESS,
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    if sys.version_info < (3, 10):
        print("BMAD verification error: Python 3.10 or newer is required", file=sys.stderr)
        return 1
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = args.root.resolve()
    if not root.is_dir():
        print(f"BMAD verification error: repository root is not a directory: {root}", file=sys.stderr)
        return 1
    try:
        Verifier(root).run()
    except VerificationError as exc:
        print(f"BMAD verification error: {exc}", file=sys.stderr)
        return 1
    print("BMAD governance verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
