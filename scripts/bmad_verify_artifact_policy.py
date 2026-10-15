"""Canonical policy contract for the human-owned artifact index."""

from __future__ import annotations

from typing import Any

from bmad_verify_common import abort, normalized_json_bytes, sha256_bytes


INDEX_FIELDS = {
    "approval_rules",
    "archive_rules",
    "artifacts",
    "authority_registry",
    "canonical_roots",
    "effectiveness_rules",
    "index_revision",
    "lifecycle_rules",
    "ownership",
    "parent_rules",
    "path_rules",
    "record_schema",
    "schema_migration_rules",
    "schema_version",
    "special_record_rules",
    "supersession_rules",
}
INDEX_DYNAMIC_FIELDS = {"artifacts", "index_revision"}
CANONICAL_INDEX_POLICY_SHA256 = (
    "e35233393a6932d505bf70759af715500d44357dca177479377a3770c5bac941"
)


def validate_index_header(index: Any) -> None:
    if not isinstance(index, dict):
        abort("invalid artifact index schema")
    if type(index.get("schema_version")) is not int or index["schema_version"] != 1:
        abort("invalid artifact index schema")
    if index.get("ownership") != "human-owned":
        abort("artifact index canonical policy is invalid")
    revision = index.get("index_revision")
    if type(revision) is not int or revision < 1:
        abort("artifact index revision is invalid")


def validate_index_policy(index: dict[str, Any]) -> None:
    if set(index) != INDEX_FIELDS:
        abort("artifact index schema fields are invalid")
    policy = {key: value for key, value in index.items() if key not in INDEX_DYNAMIC_FIELDS}
    if sha256_bytes(normalized_json_bytes(policy)) != CANONICAL_INDEX_POLICY_SHA256:
        abort("artifact index canonical policy is invalid")
