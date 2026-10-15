#!/usr/bin/env python3
"""Refresh synthetic human-owned fixture hashes after an intentional mutation."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def update_record(root: Path, record: Any) -> None:
    if not isinstance(record, dict) or not isinstance(record.get("path"), str):
        return
    record["sha256"] = digest(root / record["path"])


def main() -> None:
    root = Path(sys.argv[1])
    lock_path = root / "bmad.lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    records = lock["generation"]["inventory"].get("human_owned_files", [])
    for record in records:
        update_record(root, record)
    promotion = lock.get("repository_promotion", {})
    for record in promotion.get("human_owned_files", []):
        update_record(root, record)
    sanitization = lock.get("generation", {}).get("promotion_sanitization", {})
    update_record(root, sanitization.get("team_override"))
    lock_path.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
