"""Repository-local authority registry verification."""

from __future__ import annotations

import datetime as dt
import re
from pathlib import Path
from typing import Any

from bmad_verify_common import (
    ID_RE,
    SHA256_RE,
    abort,
    canonical_path,
    checked_target,
    exact_structure,
    load_json_file,
    path_is_within,
    scope_is_valid,
    sha256_file,
)


ROLE_KINDS = [
    {
        "artifact_approval_within_scope": True,
        "independence_exception_approval": True,
        "registry_change_approval": True,
        "role": "Don",
        "scope_eligibility": "universal",
        "trivial_exemption_approval": True,
        "urgent_security_approval": True,
    },
    {
        "artifact_approval_within_scope": True,
        "independence_exception_approval": False,
        "registry_change_approval": False,
        "role": "repository-maintainer",
        "scope_eligibility": "assigned-repository-scopes",
        "trivial_exemption_approval": True,
        "urgent_security_approval": False,
    },
    {
        "artifact_approval_within_scope": True,
        "independence_exception_approval": False,
        "registry_change_approval": False,
        "role": "scoped-maintainer",
        "scope_eligibility": "assigned-scopes",
        "trivial_exemption_approval": False,
        "urgent_security_approval": False,
    },
    {
        "artifact_approval_within_scope": True,
        "independence_exception_approval": False,
        "registry_change_approval": False,
        "role": "security-owner",
        "scope_eligibility": "assigned-security-scopes",
        "trivial_exemption_approval": False,
        "urgent_security_approval": True,
    },
]

SCOPE_RULES = {
    "canonical_root": "repository",
    "descendant_match_requires_segment_boundary": True,
    "grant_matches_descendant_scopes": True,
    "grant_matches_exact_scope": True,
    "security_owner_grants_must_be_within_security_scope": True,
    "security_scope_root": "repository/security",
    "segment_pattern": "^[a-z0-9]+(?:-[a-z0-9]+)*$",
    "segment_separator": "/",
    "universal_scope": "*",
    "universal_scope_allowed_role": "Don",
    "universal_scope_matches_all": True,
}

CHANGE_CONTROL = {
    "approval_revision_must_equal_registry_revision": True,
    "fresh_approval_evidence_digest_required": True,
    "fresh_approval_evidence_required": True,
    "fresh_don_approval_required": True,
    "identities_must_represent_verified_humans": True,
    "increment_registry_revision": True,
    "stable_ids_must_not_be_reused": True,
}

INFORMATION_BOUNDARY = {
    "repository_visible_is_confidential": False,
    "restricted_material_allowed_in_git": False,
}


def valid_calendar_date(value: Any) -> bool:
    if not isinstance(value, str) or re.fullmatch(r"\d{4}-\d{2}-\d{2}", value) is None:
        return False
    try:
        dt.date.fromisoformat(value)
    except ValueError:
        return False
    return True


class AuthorityVerifierMixin:
    def verify_authorities(self) -> None:
        authorities_path = checked_target(
            self.root, "_bmad-output/authorities.yaml", "authority registry", require_file=True
        )
        registry = load_json_file(authorities_path, "authority registry")
        required_fields = {
            "schema_version", "ownership", "registry_revision", "identity_schema",
            "registry_approval", "identities", "role_kinds", "scope_rules",
            "change_control", "information_boundary",
        }
        if (
            not isinstance(registry, dict)
            or type(registry.get("schema_version")) is not int
            or registry.get("schema_version") != 1
        ):
            abort("invalid authority registry schema")
        if set(registry) != required_fields:
            abort("authority registry declared fields are invalid")
        if registry.get("ownership") != "human-owned":
            abort("authority registry ownership is invalid")
        revision = registry.get("registry_revision")
        if type(revision) is not int or revision < 1:
            abort("authority registry revision is invalid")
        identity_schema = {
            "allowed_identity_types": ["repository-local-alias"],
            "allowed_statuses": ["active", "inactive"],
        }
        if not exact_structure(registry.get("identity_schema"), identity_schema):
            abort("authority identity schema rules are invalid")
        if not exact_structure(registry.get("role_kinds"), ROLE_KINDS):
            abort("authority role-kind rules are invalid")
        if not exact_structure(registry.get("scope_rules"), SCOPE_RULES):
            abort("authority scope rules are invalid")
        if not exact_structure(registry.get("change_control"), CHANGE_CONTROL):
            abort("authority registry change-control rules are invalid")
        if not exact_structure(registry.get("information_boundary"), INFORMATION_BOUNDARY):
            abort("authority information-boundary rules are invalid")
        self.validate_authority_identities(registry.get("identities"))
        self.validate_registry_approval(registry.get("registry_approval"), revision)

    def validate_authority_identities(self, identities: Any) -> None:
        if not isinstance(identities, list) or not identities:
            abort("invalid authority identity inventory")
        fields = {"id", "identity_type", "role", "scopes", "status", "verified_human"}
        allowed_roles = {record["role"] for record in ROLE_KINDS}
        for record in identities:
            if not isinstance(record, dict) or set(record) != fields:
                abort("invalid authority identity record: required field is missing")
            identity = record.get("id")
            if not isinstance(identity, str) or not ID_RE.fullmatch(identity):
                abort("invalid authority identity")
            if identity in self.authorities:
                abort(f"duplicate authority identity: {identity}")
            role = record.get("role")
            if not isinstance(role, str) or role not in allowed_roles:
                abort(f"invalid authority role for identity {identity}")
            if record.get("identity_type") != "repository-local-alias":
                abort(f"invalid authority identity type for {identity}")
            if record.get("verified_human") is not True:
                abort(f"authority identity is not a verified human: {identity}")
            if not isinstance(record.get("status"), str) or record.get(
                "status"
            ) not in {"active", "inactive"}:
                abort(f"invalid authority identity status for {identity}")
            scopes = record.get("scopes")
            if (
                not isinstance(scopes, list)
                or not scopes
                or not all(isinstance(scope, str) for scope in scopes)
                or len(scopes) != len(set(scopes))
                or not all(scope_is_valid(scope, allow_universal=True) for scope in scopes)
            ):
                abort(f"invalid authority scope for {identity}")
            if not self.role_scopes_are_valid(str(role), scopes):
                abort(f"authority scope constraint is invalid for {identity}")
            self.authorities[identity] = record

    def role_scopes_are_valid(self, role: str, scopes: list[Any]) -> bool:
        if role == "Don":
            return scopes == ["*"]
        if "*" in scopes or not all(isinstance(scope, str) for scope in scopes):
            return False
        root = "repository/security" if role == "security-owner" else "repository"
        return all(path_is_within(scope, root) for scope in scopes)

    def validate_registry_approval(self, approval: Any, revision: int) -> None:
        fields = {
            "approved_on", "approved_registry_revision", "approver", "approver_role",
            "evidence", "evidence_basis", "evidence_sha256", "status",
        }
        if not isinstance(approval, dict):
            abort("authority registry approval metadata is incomplete")
        if "evidence_sha256" not in approval:
            abort("invalid authority approval evidence digest")
        if set(approval) != fields:
            abort("authority registry approval metadata is incomplete")
        if (
            type(approval.get("approved_registry_revision")) is not int
            or approval.get("approved_registry_revision") != revision
            or approval.get("status") != "approved"
        ):
            abort("authority registry approval revision or status mismatch")
        if not valid_calendar_date(approval.get("approved_on")):
            abort("authority registry approval metadata has invalid approved_on")
        approver_id = approval.get("approver")
        approver = self.authorities.get(approver_id) if isinstance(approver_id, str) else None
        if (
            not approver
            or approver.get("role") != "Don"
            or approval.get("approver_role") != approver.get("role")
        ):
            abort("authority registry approval role mismatch")
        if approver.get("status") != "active" or approver.get("verified_human") is not True:
            abort("authority registry approval is not eligible")
        basis = approval.get("evidence_basis")
        if not isinstance(basis, str) or not basis.strip():
            abort("authority registry approval metadata is incomplete")
        evidence = self.validate_evidence_path(
            approval.get("evidence"), "authority approval evidence"
        )
        evidence_hash = approval.get("evidence_sha256")
        if not isinstance(evidence_hash, str) or not SHA256_RE.fullmatch(evidence_hash):
            abort("invalid authority approval evidence digest")
        if sha256_file(evidence) != evidence_hash:
            abort("authority approval evidence digest mismatch")

    def validate_evidence_path(self, raw: Any, label: str) -> Path:
        if not isinstance(raw, str) or not raw:
            abort(f"missing {label}")
        relative = canonical_path(raw, label)
        if raw != relative:
            abort(f"raw {label} path is not canonical")
        return checked_target(self.root, relative, label, require_file=True)

    def authority_is_eligible(
        self, identity: Any, scope: Any, *, roles: set[str] | None = None
    ) -> bool:
        record = self.authorities.get(identity) if isinstance(identity, str) else None
        if (
            not record
            or record.get("status") != "active"
            or record.get("verified_human") is not True
            or not scope_is_valid(scope)
        ):
            return False
        role = record.get("role")
        if roles is not None and role not in roles:
            return False
        for grant in record.get("scopes", []):
            if grant == "*" and role == "Don":
                return True
            if grant == scope or (isinstance(grant, str) and str(scope).startswith(grant + "/")):
                if role == "security-owner" and not str(scope).startswith("repository/security"):
                    continue
                return True
        return False
