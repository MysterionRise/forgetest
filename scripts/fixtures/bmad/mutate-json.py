#!/usr/bin/env python3
"""Apply one deterministic dotted-path mutation to a JSON fixture."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


def descend(value: Any, segment: str) -> Any:
    return value[int(segment)] if isinstance(value, list) else value[segment]


def main() -> None:
    path = Path(sys.argv[1])
    operation = sys.argv[2]
    segments = sys.argv[3].split(".")
    value = json.loads(path.read_text(encoding="utf-8"))
    parent = value
    for segment in segments[:-1]:
        parent = descend(parent, segment)
    final = segments[-1]
    key: int | str = int(final) if isinstance(parent, list) else final
    if operation == "delete":
        del parent[key]
    elif operation == "set":
        parent[key] = json.loads(sys.argv[4])
    else:
        raise SystemExit(f"unknown mutation operation: {operation}")
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
