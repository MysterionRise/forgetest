#!/usr/bin/env python3
"""Shared parsing, path, digest, and time helpers for BMAD verification."""

from __future__ import annotations

import ast
import datetime as dt
import hashlib
import json
import posixpath
import re
import stat
import unicodedata
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Sequence
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError


LOCKED_VERSION = "6.11.0"
RETROSPECTIVE_BUSINESS_DAY_COUNT = 5
WEEKDAY_BOUNDARY = 5
SHA512_BYTE_LENGTH = 64
START_GATE = b"      # BMAD-GOVERNANCE-GATE:START\n"
END_GATE = b"      # BMAD-GOVERNANCE-GATE:END\n"
CI_GATE_ANCHOR = b"          bash scripts/test-build-pages.sh\n"
CANONICAL_CI_GATE = b"""      # BMAD-GOVERNANCE-GATE:START
      - name: Provision exact Python
        uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7.0.0
        with:
          python-version: "3.10.0"

      - name: Build fresh Pages output
        run: bash scripts/build-pages.sh

      - name: Verify BMAD governance
        env:
          BMAD_RENDERED_OUTPUT: target/pages
        run: |
          bash scripts/test-verify-bmad.sh
          bash scripts/verify-bmad.sh
      # BMAD-GOVERNANCE-GATE:END
"""
POLICY_START = "<!-- BMAD-GOVERNANCE-POLICY:START -->"
POLICY_END = "<!-- BMAD-GOVERNANCE-POLICY:END -->"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
SHA512_RE = re.compile(r"^[0-9a-f]{128}$")
ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SCOPE_SEGMENT_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
RFC3339_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")


class VerificationError(Exception):
    """A stable, user-facing verification failure."""


class DuplicateKeyError(ValueError):
    """Raised when a JSON or narrow-YAML mapping repeats a key."""


def abort(message: str) -> None:
    raise VerificationError(message)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def no_duplicate_object(pairs: Sequence[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateKeyError(f"duplicate key {key!r}")
        result[key] = value
    return result


def load_json_bytes(data: bytes, label: str) -> Any:
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=no_duplicate_object)
    except (UnicodeDecodeError, json.JSONDecodeError, DuplicateKeyError) as exc:
        abort(f"invalid duplicate-key-safe JSON in {label}: {exc}")


def load_json_file(path: Path, label: str) -> Any:
    try:
        return load_json_bytes(path.read_bytes(), label)
    except OSError:
        abort(f"missing {label}")


def strip_yaml_comment(value: str) -> str:
    single = False
    double = False
    escaped = False
    for index, char in enumerate(value):
        if escaped:
            escaped = False
            continue
        if char == "\\" and double:
            escaped = True
            continue
        if char == "'" and not double:
            single = not single
            continue
        if char == '"' and not single:
            double = not double
            continue
        if char == "#" and not single and not double and (
            index == 0 or value[index - 1].isspace()
        ):
            return value[:index].rstrip()
    return value.rstrip()


def split_flow_items(value: str) -> list[str]:
    items: list[str] = []
    start = 0
    single = False
    double = False
    escaped = False
    depth = 0
    for index, char in enumerate(value):
        if escaped:
            escaped = False
            continue
        if char == "\\" and double:
            escaped = True
            continue
        if char == "'" and not double:
            single = not single
        elif char == '"' and not single:
            double = not double
        elif not single and not double:
            if char in "[{":
                depth += 1
            elif char in "]}":
                depth -= 1
            elif char == "," and depth == 0:
                items.append(value[start:index].strip())
                start = index + 1
    items.append(value[start:].strip())
    return items


def parse_yaml_scalar(value: str) -> Any:
    value = strip_yaml_comment(value).strip()
    if value in {"", "null", "Null", "NULL", "~"}:
        return None
    if value.lower() == "true":
        return True
    if value.lower() == "false":
        return False
    if re.fullmatch(r"-?(?:0|[1-9][0-9]*)", value):
        return int(value)
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].strip()
        return [] if not inner else [parse_yaml_scalar(item) for item in split_flow_items(inner)]
    if value.startswith("{") and value.endswith("}"):
        inner = value[1:-1].strip()
        result: dict[str, Any] = {}
        if not inner:
            return result
        for item in split_flow_items(inner):
            key, separator, raw = item.partition(":")
            if not separator:
                raise ValueError("invalid flow mapping")
            key = str(parse_yaml_scalar(key.strip()))
            if key in result:
                raise DuplicateKeyError(f"duplicate key {key!r}")
            result[key] = parse_yaml_scalar(raw)
        return result
    if (value.startswith('"') and value.endswith('"')) or (
        value.startswith("'") and value.endswith("'")
    ):
        try:
            parsed = ast.literal_eval(value)
        except (SyntaxError, ValueError) as exc:
            raise ValueError(f"invalid quoted scalar: {exc}") from exc
        if not isinstance(parsed, str):
            raise ValueError("quoted scalar is not text")
        return parsed
    return value


def split_yaml_key_value(text: str) -> tuple[str, str]:
    single = False
    double = False
    escaped = False
    for index, char in enumerate(text):
        if escaped:
            escaped = False
            continue
        if char == "\\" and double:
            escaped = True
            continue
        if char == "'" and not double:
            single = not single
        elif char == '"' and not single:
            double = not double
        elif char == ":" and not single and not double:
            key = text[:index].strip()
            if not key:
                raise ValueError("empty mapping key")
            return key, text[index + 1 :].strip()
    raise ValueError("mapping entry lacks ':'")


def load_narrow_yaml_bytes(data: bytes, label: str) -> Any:
    """Parse the supported YAML subset as one deliberately narrow operation."""
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as exc:
        abort(f"invalid UTF-8 in {label}: {exc}")

    lines: list[tuple[int, str, int]] = []
    for number, raw_line in enumerate(text.splitlines(), 1):
        if "\t" in raw_line[: len(raw_line) - len(raw_line.lstrip())]:
            abort(f"invalid narrow YAML indentation in {label} at line {number}")
        content = strip_yaml_comment(raw_line).rstrip()
        if not content.strip() or content.lstrip().startswith("#"):
            continue
        indent = len(content) - len(content.lstrip(" "))
        lines.append((indent, content.lstrip(" "), number))

    if not lines:
        return {}

    def parse_block(index: int, indent: int) -> tuple[Any, int]:
        """Recursively parse one indentation block without general YAML semantics."""
        if index >= len(lines) or lines[index][0] != indent:
            raise ValueError(f"unexpected indentation at line {lines[index][2] if index < len(lines) else 'EOF'}")
        is_list = lines[index][1].startswith("- ") or lines[index][1] == "-"
        container: Any = [] if is_list else {}
        while index < len(lines):
            current_indent, content, number = lines[index]
            if current_indent < indent:
                break
            if current_indent > indent:
                raise ValueError(f"unexpected indentation at line {number}")
            current_is_list = content.startswith("- ") or content == "-"
            if current_is_list != is_list:
                break

            if is_list:
                rest = content[1:].strip()
                index += 1
                if not rest:
                    if index >= len(lines) or lines[index][0] <= indent:
                        raise ValueError(f"empty sequence item at line {number}")
                    value, index = parse_block(index, lines[index][0])
                    container.append(value)
                    continue
                try:
                    key, raw_value = split_yaml_key_value(rest)
                except ValueError:
                    container.append(parse_yaml_scalar(rest))
                    continue
                item: dict[str, Any] = {}
                key = str(parse_yaml_scalar(key))
                if raw_value:
                    item[key] = parse_yaml_scalar(raw_value)
                elif index < len(lines) and lines[index][0] > indent:
                    item[key], index = parse_block(index, lines[index][0])
                else:
                    item[key] = None
                if index < len(lines) and lines[index][0] > indent:
                    continuation_indent = lines[index][0]
                    continuation, index = parse_block(index, continuation_indent)
                    if not isinstance(continuation, dict):
                        raise ValueError(f"sequence mapping continuation is not a mapping at line {number}")
                    for continuation_key, continuation_value in continuation.items():
                        if continuation_key in item:
                            raise DuplicateKeyError(f"duplicate key {continuation_key!r}")
                        item[continuation_key] = continuation_value
                container.append(item)
            else:
                key, raw_value = split_yaml_key_value(content)
                key = str(parse_yaml_scalar(key))
                if key in container:
                    raise DuplicateKeyError(f"duplicate key {key!r}")
                index += 1
                if raw_value:
                    container[key] = parse_yaml_scalar(raw_value)
                elif index < len(lines) and lines[index][0] > indent:
                    container[key], index = parse_block(index, lines[index][0])
                else:
                    container[key] = None
        return container, index

    try:
        parsed, final_index = parse_block(0, lines[0][0])
        if final_index != len(lines):
            raise ValueError(f"unparsed content at line {lines[final_index][2]}")
        return parsed
    except (ValueError, DuplicateKeyError) as exc:
        abort(f"invalid narrow YAML in {label}: {exc}")


def load_structured_file(path: Path, label: str) -> Any:
    try:
        data = path.read_bytes()
    except OSError:
        abort(f"missing {label}")
    if data.lstrip().startswith((b"{", b"[")):
        return load_json_bytes(data, label)
    return load_narrow_yaml_bytes(data, label)


def parse_front_matter(path: Path, label: str) -> dict[str, Any]:
    try:
        data = path.read_bytes()
    except OSError:
        abort(f"missing {label}")
    if not data.startswith(b"---\n"):
        abort(f"invalid front matter in {label}")
    end = data.find(b"\n---\n", 4)
    if end < 0:
        abort(f"unterminated front matter in {label}")
    parsed = load_narrow_yaml_bytes(data[4:end], f"{label} front matter")
    if not isinstance(parsed, dict):
        abort(f"invalid front matter mapping in {label}")
    return parsed


def parse_version_tuple(value: Any, label: str) -> tuple[int, int, int]:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", value):
        abort(f"invalid locked version in {label}")
    return tuple(int(part) for part in value.split("."))  # type: ignore[return-value]


def canonical_path(raw: Any, category: str) -> str:
    if not isinstance(raw, str) or not raw or "\x00" in raw:
        abort(f"invalid {category} path")
    if raw.startswith("/") or PurePosixPath(raw).is_absolute() or re.match(r"^[A-Za-z]:[\\/]", raw):
        abort(f"absolute {category} path is forbidden: {raw}")
    if "\\" in raw:
        abort(f"non-canonical {category} path uses a backslash: {raw}")
    segments = raw.split("/")
    if ".." in segments:
        abort(f"traversal in {category} path is forbidden: {raw}")
    canonical = posixpath.normpath(raw)
    if canonical in {"", "."} or canonical.startswith("../"):
        abort(f"traversal in {category} path is forbidden: {raw}")
    return canonical


def portable_key(path: str) -> str:
    return unicodedata.normalize("NFC", path).casefold()


def path_is_within(path: str, root: str) -> bool:
    return path == root or path.startswith(root + "/")


def validate_canonical_path_set(raw_paths: Iterable[Any], category: str) -> list[str]:
    canonical_paths: list[str] = []
    seen: dict[str, str] = {}
    raw_by_canonical: list[tuple[str, str]] = []
    for raw in raw_paths:
        canonical = canonical_path(raw, category)
        key = portable_key(canonical)
        if key in seen:
            abort(
                f"duplicate canonical {category} target: {raw!r} conflicts with {seen[key]!r}"
            )
        seen[key] = str(raw)
        canonical_paths.append(canonical)
        raw_by_canonical.append((str(raw), canonical))
    for raw, canonical in raw_by_canonical:
        if raw != canonical or "//" in raw or "/./" in f"/{raw}/":
            abort(f"non-canonical {category} path is forbidden: {raw}")
    return canonical_paths


def checked_target(root: Path, relative: str, category: str, *, require_file: bool | None = None) -> Path:
    current = root
    for segment in relative.split("/"):
        current = current / segment
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            abort(f"missing {category} target: {relative}")
        except OSError as exc:
            abort(f"unreadable {category} target {relative}: {exc}")
        if stat.S_ISLNK(metadata.st_mode):
            abort(f"symlink {category} target is forbidden: {relative}")
    if require_file is True and not current.is_file():
        abort(f"missing {category} file target: {relative}")
    if require_file is False and not current.is_dir():
        abort(f"missing {category} directory target: {relative}")
    if not current.is_file() and not current.is_dir():
        abort(f"special {category} target is forbidden: {relative}")
    return current


def normalized_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n"


def exact_structure(actual: Any, expected: Any) -> bool:
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(
            exact_structure(actual[key], value) for key, value in expected.items()
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            exact_structure(left, right) for left, right in zip(actual, expected)
        )
    return bool(actual == expected)


def scope_is_valid(scope: Any, *, allow_universal: bool = False) -> bool:
    if not isinstance(scope, str) or not scope:
        return False
    if allow_universal and scope == "*":
        return True
    return all(SCOPE_SEGMENT_RE.fullmatch(segment) for segment in scope.split("/"))


def timestamp_is_valid(value: Any) -> bool:
    if not isinstance(value, str) or not RFC3339_RE.fullmatch(value):
        return False
    try:
        dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        return False
    return True


def retrospective_deadline(approved_at: str) -> str:
    try:
        approved = dt.datetime.strptime(approved_at, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=dt.timezone.utc
        )
        london_date = approved.astimezone(ZoneInfo("Europe/London")).date()
    except (TypeError, ValueError, ZoneInfoNotFoundError) as exc:
        abort(f"unable to calculate Europe/London retrospective deadline: {exc}")
    cursor = london_date
    count = 0
    while count < RETROSPECTIVE_BUSINESS_DAY_COUNT:
        cursor += dt.timedelta(days=1)
        if cursor.weekday() < WEEKDAY_BOUNDARY:
            count += 1
    return cursor.isoformat()
