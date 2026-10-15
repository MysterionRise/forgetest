#!/usr/bin/env python3
"""Apply one deliberately malformed placement to the fixture CI gate."""

from __future__ import annotations

import sys
from pathlib import Path


START = b"      # BMAD-GOVERNANCE-GATE:START\n"
END = b"      # BMAD-GOVERNANCE-GATE:END\n"


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: mutate-ci-gate.py PATH MODE")
    path = Path(sys.argv[1])
    mode = sys.argv[2]
    data = path.read_bytes()
    start = data.index(START)
    finish = data.index(END) + len(END)
    block = data[start:finish]
    baseline = data[:start] + data[finish:]
    if mode == "moved":
        mutated = baseline + block
    elif mode == "root":
        mutated = data[:start] + block.replace(b"      ", b"", 1) + data[finish:]
    elif mode == "nested":
        mutated = data[:start] + b"  " + block.replace(b"\n", b"\n  ")[:-2] + data[finish:]
    elif mode == "reversed":
        body = block[len(START) : -len(END)]
        mutated = data[:start] + END + body + START + data[finish:]
    else:
        raise SystemExit(f"unknown mode: {mode}")
    path.write_bytes(mutated)


if __name__ == "__main__":
    main()
