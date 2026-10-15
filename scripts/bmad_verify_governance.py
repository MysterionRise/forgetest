"""Authority registry and durable artifact lifecycle verification."""

from __future__ import annotations

import datetime as dt
import re
from pathlib import Path
from typing import Any

from bmad_verify_artifact_policy import validate_index_header, validate_index_policy
from bmad_verify_common import (
    ID_RE,
    abort,
    canonical_path,
    checked_target,
    load_json_file,
    parse_front_matter,
    path_is_within,
    retrospective_deadline,
    scope_is_valid,
    timestamp_is_valid,
    validate_canonical_path_set,
)


class GovernanceVerifierMixin:
    def verify_authorities_and_artifacts(self) -> None:
        self.verify_authorities()

        index_path = checked_target(
            self.root, "_bmad-output/index.yaml", "artifact index", require_file=True
        )
        index = load_json_file(index_path, "artifact index")
        validate_index_header(index)
        if index.get("authority_registry") != "_bmad-output/authorities.yaml":
            abort("artifact authority registry path is invalid")
        artifacts = index.get("artifacts")
        if not isinstance(artifacts, list):
            abort("invalid artifact record inventory")
        self.index = index
        self.artifacts = artifacts
        self.validate_artifacts()

    def validate_artifacts(self) -> None:
        """Validate one artifact graph in contract-significant fail-fast order."""
        allowed_statuses = {"draft", "approved", "superseded", "archived"}
        allowed_types = {
            "plan", "product-brief", "prd", "architecture", "ux-design", "research",
            "retrospective", "epic", "story", "trivial-exemption",
            "urgent-security-incident", "schema-migration",
        }
        allowed_classes = {"public", "repository-visible"}
        expected_roots = [
            "_bmad-output/planning-artifacts",
            "_bmad-output/implementation-artifacts",
        ]
        canonical_roots = self.index.get("canonical_roots")
        if canonical_roots != expected_roots:
            abort("invalid artifact canonical roots")
        canonical_roots = validate_canonical_path_set(canonical_roots, "artifact root")
        validate_index_policy(self.index)
        for root in canonical_roots:
            checked_target(self.root, root, "artifact root", require_file=False)
        records: dict[str, dict[str, Any]] = {}
        effective: dict[tuple[str, str], str] = {}
        indexed_paths: set[str] = set()
        required_record_fields = {
            "id",
            "artifact_type",
            "title",
            "path",
            "status",
            "owner",
            "approver",
            "approved_at",
            "approval_evidence",
            "scope",
            "parent",
            "supersedes",
            "superseded_by",
            "supersession_authorizer",
            "superseded_at",
            "supersession_evidence",
            "classification",
            "independence_exception_approver",
            "independence_exception_approved_at",
            "independence_exception_evidence",
            "independence_exception_rationale",
            "archive_authorizer",
            "archived_at",
            "archive_evidence",
        }
        record_schema = self.index.get("record_schema")
        if record_schema is not None:
            schema_fields = (
                record_schema.get("required_fields")
                if isinstance(record_schema, dict)
                else None
            )
            if (
                not isinstance(schema_fields, list)
                or not all(isinstance(field, str) for field in schema_fields)
                or set(schema_fields) != required_record_fields
            ):
                abort("artifact record schema required fields are invalid")

        for record in self.artifacts:
            if not isinstance(record, dict):
                abort("invalid artifact record")
            if not required_record_fields.issubset(record):
                abort("artifact record schema required field is missing")
            if set(record) != required_record_fields:
                abort("artifact record schema unexpected field is present")
            artifact_id = record.get("id")
            if not isinstance(artifact_id, str) or not ID_RE.fullmatch(artifact_id):
                abort("invalid artifact identity")
            string_fields = {
                "artifact_type", "title", "path", "status", "owner", "scope",
                "classification",
            }
            nullable_string_fields = required_record_fields - string_fields - {"id"}
            if any(not isinstance(record.get(field), str) for field in string_fields) or any(
                record.get(field) is not None
                and not isinstance(record.get(field), str)
                for field in nullable_string_fields
            ):
                abort(f"artifact record field type is invalid: {artifact_id}")
            if artifact_id in records:
                abort(f"duplicate artifact identity: {artifact_id}")
            records[artifact_id] = record
            artifact_type = record.get("artifact_type")
            if artifact_type not in allowed_types:
                abort(f"invalid artifact type for {artifact_id}")
            status_value = record.get("status")
            if status_value not in allowed_statuses:
                abort(f"invalid artifact lifecycle for {artifact_id}")
            classification = record.get("classification")
            if classification not in allowed_classes:
                abort(f"invalid artifact classification for {artifact_id}")
            if not isinstance(record.get("title"), str) or not record["title"].strip():
                abort(f"invalid artifact title for {artifact_id}")
            if not scope_is_valid(record.get("scope")):
                abort(f"invalid artifact scope for {artifact_id}")
            if record.get("owner") not in self.authorities:
                abort(f"invalid artifact owner identity for {artifact_id}")

            raw_path = record.get("path")
            relative = canonical_path(raw_path, "artifact")
            if raw_path != relative:
                abort("raw artifact path is not canonical")
            if not any(path_is_within(relative, root) for root in canonical_roots):
                abort(f"artifact path is outside a canonical root: {relative}")
            if Path(relative).suffix != ".md" or Path(relative).stem != artifact_id:
                abort(f"artifact path does not match artifact identity: {artifact_id}")
            if relative in indexed_paths:
                abort(f"canonical artifact path is indexed more than once: {relative}")
            indexed_paths.add(relative)
            checked_target(self.root, relative, "artifact", require_file=True)

            if status_value == "draft":
                for field in (
                    "approver", "approved_at", "approval_evidence", "supersedes",
                    "superseded_by", "supersession_authorizer", "superseded_at",
                    "supersession_evidence", "independence_exception_approver",
                    "independence_exception_approved_at", "independence_exception_evidence",
                    "independence_exception_rationale", "archive_authorizer", "archived_at",
                    "archive_evidence",
                ):
                    if record.get(field) is not None:
                        abort(f"draft artifact lifecycle field must be null: {artifact_id}")
            else:
                approver_id = record.get("approver")
                if not self.authority_is_eligible(approver_id, record.get("scope")):
                    abort(f"artifact approver is not eligible: {artifact_id}")
                if not timestamp_is_valid(record.get("approved_at")):
                    abort(f"invalid artifact approval timestamp: {artifact_id}")
                self.validate_evidence_path(
                    record.get("approval_evidence"), f"artifact approval evidence for {artifact_id}"
                )
                if record.get("owner") == approver_id:
                    self.validate_independence_exception(record)
                elif any(
                    record.get(field) is not None
                    for field in (
                        "independence_exception_approver",
                        "independence_exception_approved_at",
                        "independence_exception_evidence",
                        "independence_exception_rationale",
                    )
                ):
                    abort(f"artifact independence exception fields must be null: {artifact_id}")

            if status_value == "approved":
                for field in (
                    "superseded_by", "supersession_authorizer", "superseded_at",
                    "supersession_evidence", "archive_authorizer", "archived_at", "archive_evidence",
                ):
                    if record.get(field) is not None:
                        abort(f"approved artifact lifecycle field must be null: {artifact_id}")
                key = (str(artifact_type), str(record.get("scope")))
                if key in effective:
                    abort(f"duplicate effective artifact: {artifact_id} conflicts with {effective[key]}")
                effective[key] = artifact_id
            elif status_value == "superseded":
                if not record.get("superseded_by"):
                    abort(f"supersession successor is missing: {artifact_id}")
                self.validate_transition_authority(record, "supersession")
            elif status_value == "archived":
                if not record.get("archive_authorizer") or not record.get("archive_evidence"):
                    abort(f"archive authorization is missing: {artifact_id}")
                self.validate_transition_authority(record, "archive")
                supersession_group = (
                    "superseded_by",
                    "supersession_authorizer",
                    "superseded_at",
                    "supersession_evidence",
                )
                retained = [record.get(field) is not None for field in supersession_group]
                if any(retained) and not all(retained):
                    abort(
                        "archived artifact supersession fields are incomplete: "
                        f"{artifact_id}"
                    )
                if all(retained):
                    self.validate_transition_authority(record, "supersession")
                    if self.timestamp_value(record.get("superseded_at")) > self.timestamp_value(
                        record.get("archived_at")
                    ):
                        abort(f"artifact archive chronology is invalid: {artifact_id}")

            if artifact_type == "trivial-exemption" and status_value != "draft":
                role = self.authorities[str(record.get("approver"))].get("role")
                if role not in {"repository-maintainer", "Don"}:
                    abort(f"artifact approver is not eligible for trivial exemption: {artifact_id}")
                self.require_artifact_content(relative, artifact_id, ["rationale"])
            if artifact_type == "urgent-security-incident" and status_value != "draft":
                role = self.authorities[str(record.get("approver"))].get("role")
                if role not in {"security-owner", "Don"}:
                    abort(f"artifact approver is not eligible for urgent security: {artifact_id}")
                self.validate_incident_deadline(relative, record)

        for artifact_id, record in records.items():
            parent = record.get("parent")
            if parent is not None:
                parent_record = records.get(parent)
                if parent_record is None:
                    abort(f"artifact parent does not exist: {artifact_id}")
                artifact_type = record.get("artifact_type")
                parent_type = parent_record.get("artifact_type")
                if artifact_type == "story" and parent_type not in {"plan", "epic"}:
                    abort(f"artifact parent type is invalid: {artifact_id}")
                if artifact_type == "epic" and parent_type != "plan":
                    abort(f"artifact parent type is invalid: {artifact_id}")
                parent_scope = str(parent_record.get("scope"))
                child_scope = str(record.get("scope"))
                if child_scope != parent_scope and not child_scope.startswith(parent_scope + "/"):
                    abort(f"artifact parent scope does not cover child: {artifact_id}")
                if record.get("status") == "approved" and parent_record.get("status") == "archived":
                    abort(f"effective artifact has archived parent: {artifact_id}")
            elif record.get("artifact_type") == "story":
                abort(f"story artifact parent is required: {artifact_id}")

            predecessor_id = record.get("supersedes")
            if predecessor_id is not None:
                predecessor = records.get(predecessor_id)
                if predecessor is None:
                    abort(f"artifact supersession target does not exist: {artifact_id}")
                if predecessor.get("artifact_type") != record.get("artifact_type") or predecessor.get("scope") != record.get("scope"):
                    abort(f"artifact supersession type or scope mismatch: {artifact_id}")
                if predecessor.get("superseded_by") != artifact_id:
                    abort(f"artifact supersession links are not bidirectional: {artifact_id}")
            successor_id = record.get("superseded_by")
            if successor_id is not None:
                successor = records.get(successor_id)
                if successor is None or successor.get("supersedes") != artifact_id:
                    abort(f"artifact supersession links are not bidirectional: {artifact_id}")
                if self.timestamp_value(successor.get("approved_at")) > self.timestamp_value(
                    record.get("superseded_at")
                ):
                    abort(f"artifact supersession chronology is invalid: {artifact_id}")

        self.reject_artifact_cycles(records, "parent", "parent")
        self.reject_artifact_cycles(records, "supersedes", "supersession")
        self.verify_artifact_inventory(canonical_roots, indexed_paths)

    def validate_independence_exception(self, record: dict[str, Any]) -> None:
        artifact_id = str(record.get("id"))
        approver = record.get("independence_exception_approver")
        authority = self.authorities.get(approver)
        if not authority or authority.get("role") != "Don" or not self.authority_is_eligible(
            approver, record.get("scope")
        ):
            abort(f"artifact owner and approver conflict lacks Don exception: {artifact_id}")
        if not timestamp_is_valid(record.get("independence_exception_approved_at")):
            abort(f"artifact owner and approver conflict has invalid exception time: {artifact_id}")
        if self.timestamp_value(record.get("independence_exception_approved_at")) > self.timestamp_value(
            record.get("approved_at")
        ):
            abort(f"artifact independence exception chronology is invalid: {artifact_id}")
        if not isinstance(record.get("independence_exception_rationale"), str) or not record[
            "independence_exception_rationale"
        ].strip():
            abort(f"artifact owner and approver conflict lacks rationale: {artifact_id}")
        self.validate_evidence_path(
            record.get("independence_exception_evidence"),
            f"artifact owner exception evidence for {artifact_id}",
        )

    def validate_transition_authority(self, record: dict[str, Any], transition: str) -> None:
        artifact_id = str(record.get("id"))
        authorizer_field = f"{transition}_authorizer"
        timestamp_field = "archived_at" if transition == "archive" else "superseded_at"
        evidence_field = f"{transition}_evidence"
        if not self.authority_is_eligible(record.get(authorizer_field), record.get("scope")):
            abort(f"{transition} authorizer is not eligible: {artifact_id}")
        if not timestamp_is_valid(record.get(timestamp_field)):
            abort(f"{transition} timestamp is invalid: {artifact_id}")
        if self.timestamp_value(record.get(timestamp_field)) < self.timestamp_value(
            record.get("approved_at")
        ):
            abort(f"artifact {transition} chronology is invalid: {artifact_id}")
        self.validate_evidence_path(record.get(evidence_field), f"{transition} evidence for {artifact_id}")

    def timestamp_value(self, value: Any) -> dt.datetime:
        return dt.datetime.strptime(str(value), "%Y-%m-%dT%H:%M:%SZ")

    def reject_artifact_cycles(
        self, records: dict[str, dict[str, Any]], field: str, label: str
    ) -> None:
        for start in records:
            seen: set[str] = set()
            current: str | None = start
            while current is not None:
                if current in seen:
                    abort(f"artifact {label} cycle detected: {start}")
                seen.add(current)
                target = records[current].get(field)
                current = str(target) if target is not None else None

    def verify_artifact_inventory(self, roots: list[str], indexed: set[str]) -> None:
        for root in roots:
            root_path = self.root / root
            for path in sorted(root_path.rglob("*.md")):
                if path.is_symlink():
                    abort(f"symlink artifact target is forbidden: {path.relative_to(self.root).as_posix()}")
                relative = path.relative_to(self.root).as_posix()
                if path.parent == root_path and path.name == "README.md":
                    continue
                if relative not in indexed:
                    abort(f"unindexed canonical artifact: {relative}")

    def require_artifact_content(self, relative: str, artifact_id: str, fields: list[str]) -> dict[str, Any]:
        path = self.root / relative
        try:
            data = path.read_bytes()
            text = data.decode("utf-8")
        except (OSError, UnicodeError):
            abort(f"unable to read shared artifact instruction surface: {relative}")
        if data.startswith(b"---\n"):
            content = parse_front_matter(path, f"artifact {artifact_id}")
        else:
            content = {}
            for field in fields:
                match = re.search(rf"(?mi)^\s*{re.escape(field)}\s*:\s*(.+?)\s*$", text)
                if match:
                    content[field] = match.group(1).strip()
        for field in fields:
            if content.get(field) in (None, ""):
                abort(f"artifact {artifact_id} is missing required content: {field}")
        return content

    def validate_incident_deadline(self, relative: str, record: dict[str, Any]) -> None:
        fields = [
            "incident_scope", "retrospective_deadline", "retrospective_deadline_timezone",
            "retrospective_deadline_rule",
        ]
        content = self.require_artifact_content(relative, str(record.get("id")), fields)
        if content["retrospective_deadline_timezone"] != "Europe/London":
            abort(f"urgent security deadline timezone is invalid: {record.get('id')}")
        expected_rule = "Next five Monday-through-Friday calendar days with no holiday exclusions."
        if content["retrospective_deadline_rule"] != expected_rule:
            abort(f"urgent security retrospective deadline rule is invalid: {record.get('id')}")
        if content["incident_scope"] != record.get("scope"):
            abort(f"urgent security incident scope is invalid: {record.get('id')}")
        expected_deadline = retrospective_deadline(str(record["approved_at"]))
        if str(content["retrospective_deadline"]) != expected_deadline:
            abort(f"urgent security retrospective deadline is invalid: {record.get('id')}")
