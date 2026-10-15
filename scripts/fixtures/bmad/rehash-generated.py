#!/usr/bin/env python3
"""Refresh synthetic fixture hashes after an intentional generated-file mutation."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    root = Path(sys.argv[1])
    lock_path = root / "bmad.lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    inventory = lock["generation"]["inventory"]
    hashes = inventory["generated_files_sha256"]
    for relative in hashes:
        hashes[relative] = digest(root / relative)
    payload = {
        "generated_paths": inventory["generated_paths"],
        "generated_files_sha256": hashes,
        "symlinks": [],
    }
    encoded = json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8") + b"\n"
    tree_digest = hashlib.sha256(encoded).hexdigest()
    inventory["raw_promotion_sha256"] = tree_digest
    reproducibility = lock["generation"]["reproducibility"]
    reproducibility["first_raw_promotion_sha256"] = tree_digest
    reproducibility["second_raw_promotion_sha256"] = tree_digest
    promotion = lock["repository_promotion"]
    promotion["source_raw_generated_tree_sha256"] = tree_digest
    promotion["promoted_raw_generated_tree_sha256"] = tree_digest
    lock_path.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
