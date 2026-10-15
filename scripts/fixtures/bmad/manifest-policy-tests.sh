#!/usr/bin/env bash

policy_joint_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token "$fixture_root/AGENTS.md" "$old" "$new"
  replace_token "$fixture_root/CLAUDE.md" "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

file_manifest_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad/_config/files-manifest.csv" "$old" "$new"
  python3 "$FIXTURES/rehash-generated.py" "$fixture_root"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_manifest_and_policy_guards() {
  local fixture_launcher="$BASE/.agents/skills/bmad-build/SKILL.md"
  [ -f "$fixture_launcher" ] || fail "fixture launcher is not production-shaped"
  ! grep -REq "workflow_id|module_version|authority_precedence|study_prohibition" \
    "$BASE/.agents" "$BASE/fixture-claude" || \
    fail "fixture launcher retains production-dead front matter"
  grep -Fq "name: bmad-build" "$fixture_launcher" || \
    fail "fixture launcher lacks production launcher identity"
  grep -Fq "uv run --no-cache" "$fixture_launcher" || \
    fail "fixture launcher lacks production renderer contract"
  grep -Fq '"bmad-build"' "$BASE/_bmad/_config/skill-manifest.csv" || \
    fail "fixture skill manifest lacks production-shaped workflow identity"
  printf 'PASS production-shaped-launcher-fixture\n'

  file_manifest_guard \
    "file-manifest-launcher-hash" \
    "bce4778d8d275095ca1836733337163c1bf11a5a62e6585b951de458a5ac2c8e" \
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
    "file manifest launcher hash"
  file_manifest_guard \
    "file-manifest-skill-path" \
    "bmm/ship/bmad-build/SKILL.md" \
    "bmm/ship/bmad-missing/SKILL.md" \
    "file manifest skill mapping"

  policy_joint_guard \
    "policy-tier-order" '"order": 1,' '"order": 4,' \
    "repository instruction workflow tiers"
  policy_joint_guard \
    "policy-forbidden-data" '"credentials",' '"credentials-removed",' \
    "repository instruction forbidden-data policy"
  policy_joint_guard \
    "policy-upgrade" \
    '"working_checkout_installer_or_regeneration_allowed": false' \
    '"working_checkout_installer_or_regeneration_allowed": true' \
    "repository instruction upgrade policy"
  policy_joint_guard \
    "policy-publication" '"review_required": true' '"review_required": false' \
    "repository instruction publication policy"
  policy_joint_guard \
    "policy-protected-study" '"mutation_allowed": false' '"mutation_allowed": true' \
    "repository instruction protected-study policy"
  policy_joint_guard \
    "policy-authoritative-paths" '"paths": [' '"paths_removed": [' \
    "repository instruction authority precedence"
  policy_joint_guard \
    "policy-unexpected-field" '"policy_schema_version": 1,' \
    '"policy_schema_version": 1, "unexpected": true,' \
    "repository instruction machine policy"
}
