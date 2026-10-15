#!/usr/bin/env bash

authority_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/authorities.yaml" "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_authority_schema_guards() {
  authority_guard \
    "authority-schema-exact-integer" \
    '"schema_version": 1' '"schema_version": true' \
    "authority registry schema"
  authority_guard \
    "authority-declared-field-missing" \
    '"change_control": {' '"change_control_removed": {' \
    "authority registry declared fields"
  authority_guard \
    "authority-change-control-rule" \
    '"fresh_approval_evidence_digest_required": true' \
    '"fresh_approval_evidence_digest_required": false' \
    "authority registry change-control rules"
  authority_guard \
    "authority-identity-schema-rule" \
    '"allowed_statuses": ["active", "inactive"]' \
    '"allowed_statuses": ["active"]' \
    "authority identity schema rules"
  authority_guard \
    "authority-role-kind-rule" \
    '"scope_eligibility": "assigned-security-scopes"' \
    '"scope_eligibility": "assigned-scopes"' \
    "authority role-kind rules"
  authority_guard \
    "authority-security-scope-rule" \
    '"scopes": ["repository/security"]' \
    '"scopes": ["repository"]' \
    "authority scope constraint"
  authority_guard \
    "authority-approval-date-missing" \
    '"approved_on": "2026-08-14"' \
    '"approved_on_removed": "2026-08-14"' \
    "authority registry approval metadata"
  authority_guard \
    "authority-approver-role-mismatch" \
    '"approver_role": "Don"' \
    '"approver_role": "repository-maintainer"' \
    "authority registry approval role mismatch"
  authority_guard \
    "authority-evidence-digest-missing" \
    '"evidence_sha256": "737ce61bd16ed9e23a0e0df9b6e6645731f7937c12a4113e8288367915a72b6f"' \
    '"evidence_sha256_removed": "737ce61bd16ed9e23a0e0df9b6e6645731f7937c12a4113e8288367915a72b6f"' \
    "authority approval evidence digest"
  authority_guard \
    "authority-evidence-digest-mismatch" \
    '737ce61bd16ed9e23a0e0df9b6e6645731f7937c12a4113e8288367915a72b6f' \
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' \
    "authority approval evidence digest mismatch"

  fixture_root="$TEST_ROOT/guard-authority-nested-scope"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/authorities.yaml" \
    '"scopes": ["repository/security"]' \
    '"scopes": [["repository/security"]]'
  expect_exact_rejection "authority-nested-scope" "$fixture_root" \
    "BMAD verification error: invalid authority scope for security-1"
}
