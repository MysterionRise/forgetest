#!/usr/bin/env bash

incident_index_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/index.yaml" "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

incident_content_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token \
    "$fixture_root/_bmad-output/planning-artifacts/urgent-security-incident-001.md" \
    "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_urgent_incident_guards() {
  incident_index_guard \
    "urgent-incident-approver-role" \
    '"approver": "security-1"' '"approver": "maintainer-1"' \
    "urgent security"
  incident_content_guard \
    "urgent-incident-required-content" \
    'incident_scope: repository/security/incident-001' \
    'incident_scope_removed: repository/security/incident-001' \
    "missing required content: incident_scope"
  incident_content_guard \
    "urgent-incident-timezone" \
    'retrospective_deadline_timezone: Europe/London' \
    'retrospective_deadline_timezone: UTC' \
    "urgent security deadline timezone"
  incident_content_guard \
    "urgent-incident-deadline-rule" \
    'retrospective_deadline_rule: Next five Monday-through-Friday calendar days with no holiday exclusions.' \
    'retrospective_deadline_rule: Five business days.' \
    "urgent security retrospective deadline rule"
  incident_content_guard \
    "urgent-incident-scope-match" \
    'incident_scope: repository/security/incident-001' \
    'incident_scope: repository/security/other-incident' \
    "urgent security incident scope"
  incident_content_guard \
    "urgent-incident-deadline" \
    'retrospective_deadline: "2026-09-07"' \
    'retrospective_deadline: "2026-09-08"' \
    "urgent security retrospective deadline"
}
