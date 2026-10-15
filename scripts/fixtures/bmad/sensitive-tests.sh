#!/usr/bin/env bash

expect_sensitive_rejection() {
  local case_name="$1" fixture_root="$2" target="$3" token="$4" output
  track_fixture "$fixture_root"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted synthetic sensitive content: $case_name"
  fi
  if ! grep -Fq "sensitive content detected in $target" <<<"$output"; then
    printf '%s\n' "$output" >&2
    fail "sensitive-content guard lacked stable path diagnostic: $case_name"
  fi
  if grep -Fq -- "$token" <<<"$output"; then
    fail "sensitive-content diagnostic echoed detected value: $case_name"
  fi
  printf 'PASS %s\n' "$case_name"
}

sensitive_append_guard() {
  local case_name="$1" target="$2" token="$3" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  mkdir -p "$(dirname "$fixture_root/$target")"
  [ -f "$fixture_root/$target" ] || : >"$fixture_root/$target"
  printf '\n%s\n' "$token" >>"$fixture_root/$target"
  expect_sensitive_rejection "$case_name" "$fixture_root" "$target" "$token"
}

sensitive_generated_guard() {
  local case_name="$1" target="$2" token="$3" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  printf '\n%s\n' "$token" >>"$fixture_root/$target"
  python3 "$FIXTURES/rehash-generated.py" "$fixture_root"
  expect_sensitive_rejection "$case_name" "$fixture_root" "$target" "$token"
}

sensitive_launcher_guard() {
  local token="$1" fixture_root="$TEST_ROOT/guard-sensitive-launcher" root target new_hash
  copy_base "$fixture_root"
  for root in .agents .claude; do
    target="$root/skills/bmad-build/SKILL.md"
    printf '\n%s\n' "$token" >>"$fixture_root/$target"
  done
  new_hash="$(sha256_file "$fixture_root/.agents/skills/bmad-build/SKILL.md")"
  replace_token "$fixture_root/_bmad/_config/files-manifest.csv" \
    "bce4778d8d275095ca1836733337163c1bf11a5a62e6585b951de458a5ac2c8e" \
    "$new_hash"
  python3 "$FIXTURES/rehash-generated.py" "$fixture_root"
  expect_sensitive_rejection \
    "sensitive-launcher" "$fixture_root" ".agents/skills/bmad-build/SKILL.md" "$token"
}

sensitive_human_owned_guard() {
  local token="$1" fixture_root="$TEST_ROOT/guard-sensitive-team-config"
  local target="_bmad/custom/config.toml"
  copy_base "$fixture_root"
  printf '\n%s\n' "$token" >>"$fixture_root/$target"
  python3 "$FIXTURES/rehash-human-owned.py" "$fixture_root"
  expect_sensitive_rejection "sensitive-team-config" "$fixture_root" "$target" "$token"
}

sensitive_staged_blob_guard() {
  local fixture_root="$TEST_ROOT/guard-sensitive-staged-output-blob"
  local target="_bmad-output/project-knowledge/staged.txt"
  local token="SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY" output
  copy_base "$fixture_root"
  mkdir -p "$(dirname "$fixture_root/$target")"
  printf '%s\n' "$token" >"$fixture_root/$target"
  track_fixture "$fixture_root"
  printf 'safe working-tree replacement\n' >"$fixture_root/$target"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted sensitive staged BMAD output blob"
  fi
  grep -Fq "sensitive content detected in $target" <<<"$output" || {
    printf '%s\n' "$output" >&2
    fail "staged sensitive-content guard lacked stable path diagnostic"
  }
  if grep -Fq -- "$token" <<<"$output"; then
    fail "staged sensitive-content diagnostic echoed detected value"
  fi
  printf 'PASS sensitive-staged-output-blob\n'
}

verify_sensitive_content_guards() {
  local target="_bmad-output/planning-artifacts/plan-001.md" fixture_root token
  sensitive_append_guard \
    "sensitive-sentinel-detector" "$target" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_append_guard \
    "sensitive-private-key-detector" "$target" \
    "-----BEGIN PRIVATE KEY-----"
  sensitive_append_guard \
    "sensitive-aws-detector" "$target" \
    "AKIA0000000000000000"
  sensitive_append_guard \
    "sensitive-github-detector" "$target" \
    "ghp_000000000000000000000000000000"
  sensitive_append_guard \
    "sensitive-api-key-detector" "$target" \
    "sk-SYNTHETIC-NONLIVE-000000000000"
  sensitive_append_guard \
    "sensitive-credentials-label-detector" "$target" \
    "credentials: true"
  sensitive_append_guard \
    "sensitive-secrets-label-detector" "$target" \
    "secrets: yes"
  sensitive_append_guard \
    "sensitive-personal-data-detector" "$target" \
    "personal-data: true"
  sensitive_append_guard \
    "sensitive-customer-detector" "$target" \
    "customer-confidential: true"
  sensitive_append_guard \
    "sensitive-customer-information-detector" "$target" \
    "customer-confidential-information: yes"
  sensitive_append_guard \
    "sensitive-private-evidence-detector" "$target" \
    "raw-private-evidence: yes"
  sensitive_append_guard \
    "sensitive-live-exploit-detector" "$target" \
    "live-exploit-material: true"
  sensitive_append_guard \
    "sensitive-restricted-storage-detector" "$target" \
    "restricted-storage-material: true"

  sensitive_generated_guard \
    "sensitive-framework" "_bmad/workflows/plan.md" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_launcher_guard \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_human_owned_guard \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_append_guard \
    "sensitive-artifact-readme" "_bmad-output/planning-artifacts/README.md" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_append_guard \
    "sensitive-approval-evidence" "_bmad-output/governance/incident-approval.md" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_append_guard \
    "sensitive-bootstrap-evidence" \
    "docs/gangsta/bmad-repository-setup/evidence/synthetic.md" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"
  sensitive_append_guard \
    "sensitive-arbitrary-tracked-bmad-output" \
    "_bmad-output/project-knowledge/arbitrary.txt" \
    "SYNTHETIC_PROHIBITED_SENSITIVE_CONTENT_MARKER_FOR_TEST_ONLY"

  fixture_root="$TEST_ROOT/guard-sensitive-tracked-output-symlink-blob"
  target="_bmad-output/project-knowledge/sensitive-link"
  token="AKIA0000000000000000"
  copy_base "$fixture_root"
  mkdir -p "$(dirname "$fixture_root/$target")"
  ln -s "$token" "$fixture_root/$target"
  expect_sensitive_rejection \
    "sensitive-tracked-output-symlink-blob" "$fixture_root" "$target" "$token"
  sensitive_staged_blob_guard
}
