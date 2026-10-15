#!/usr/bin/env bash
set -euo pipefail
unset BMAD_RENDERED_OUTPUT

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFIER="$ROOT/scripts/verify-bmad.sh"
FIXTURES="$ROOT/scripts/fixtures/bmad"
BASE="$FIXTURES/base"
MATRIX="$FIXTURES/matrix.tsv"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgetest-bmad-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

fail() {
  printf 'BMAD governance test error: %s\n' "$1" >&2
  exit 1
}

. "$FIXTURES/runtime-tests.sh"
. "$FIXTURES/lock-tests.sh"
. "$FIXTURES/local-state-tests.sh"
. "$FIXTURES/manifest-policy-tests.sh"
. "$FIXTURES/authority-tests.sh"
. "$FIXTURES/artifact-tests.sh"
. "$FIXTURES/incident-tests.sh"
. "$FIXTURES/sensitive-tests.sh"
. "$FIXTURES/study-tests.sh"
. "$FIXTURES/ci-publication-tests.sh"

normalize_group() {
  case "$1" in
    all|lock-and-paths|authority-and-artifacts|launchers-studies-publication|context-and-ci-drift)
      printf '%s\n' "$1"
      ;;
    disposable-inventory|generated-install)
      printf 'lock-and-paths\n'
      ;;
    artifact-governance)
      printf 'authority-and-artifacts\n'
      ;;
    repository-instructions|onboarding)
      printf 'launchers-studies-publication\n'
      ;;
    project-context)
      printf 'context-and-ci-drift\n'
      ;;
    *)
      fail "unknown case group: $1"
      ;;
  esac
}

SELECTED_GROUP="all"
if [ "$#" -gt 0 ]; then
  [ "$#" -eq 2 ] && [ "$1" = "--case" ] || \
    fail "usage: scripts/test-verify-bmad.sh [--case CASE_GROUP]"
  SELECTED_GROUP="$(normalize_group "$2")"
fi

validate_matrix() {
  [ -d "$BASE" ] || fail "missing base fixture"
  [ -f "$MATRIX" ] || fail "missing fixture matrix"
  awk -F '\t' '
    NF != 7 { exit 1 }
    $1 !~ /^(lock-and-paths|authority-and-artifacts|launchers-studies-publication|context-and-ci-drift)$/ { exit 1 }
    seen[$2]++ { exit 1 }
    END { if (NR != 32) exit 1 }
  ' "$MATRIX" || fail "invalid fixture matrix"
}

replace_token() {
  local file="$1" old="$2" new="$3" temporary="$1.mutating"
  awk -v old="$old" -v new="$new" '
    {
      position = index($0, old)
      if (position > 0) {
        count++
        $0 = substr($0, 1, position - 1) new substr($0, position + length(old))
      }
      print
    }
    END { if (count != 1) exit 1 }
  ' "$file" >"$temporary" || fail "fixture token is not unique in ${file#$ROOT/}"
  mv "$temporary" "$file"
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

apply_mutation() {
  local fixture_root="$1" operation="$2" target="$3" first="$4" second="$5"
  local target_path="$fixture_root/$target" payload="$FIXTURES/payloads/$first"
  case "$target" in
    /*|../*|*/../*) fail "unsafe fixture mutation target: $target" ;;
  esac
  case "$operation" in
    replace)
      [ -f "$target_path" ] || fail "missing replacement target: $target"
      replace_token "$target_path" "$first" "$second"
      ;;
    replace-all)
      [ -f "$target_path" ] || fail "missing replacement target: $target"
      awk -v old="$first" -v new="$second" '
        { count += gsub(old, new); print }
        END { if (count < 1) exit 1 }
      ' "$target_path" >"$target_path.mutating" || \
        fail "fixture token is absent in ${target_path#$ROOT/}"
      mv "$target_path.mutating" "$target_path"
      ;;
    append)
      [ -f "$target_path" ] || fail "missing append target: $target"
      [ -f "$payload" ] || fail "missing fixture payload: $first"
      printf '\n' >>"$target_path"
      sed -n '1,$p' "$payload" >>"$target_path"
      ;;
    create)
      [ -f "$payload" ] || fail "missing fixture payload: $first"
      mkdir -p "$(dirname "$target_path")"
      cp "$payload" "$target_path"
      ;;
    symlink)
      [ -e "$target_path" ] || fail "missing symlink target path: $target"
      rm -f "$target_path"
      ln -s "$first" "$target_path"
      ;;
    rehash)
      [ -f "$target_path" ] || fail "missing rehash target: $target"
      replace_token "$fixture_root/bmad.lock.json" \
        "\"$target\": \"$first\"" \
        "\"$target\": \"$(sha256_file "$target_path")\""
      ;;
    rehash-context)
      [ -f "$target_path" ] || fail "missing context rehash target: $target"
      replace_token "$fixture_root/_bmad-output/project-context.md" \
        "$first" "$(sha256_file "$target_path")"
      ;;
    batch)
      [ -f "$payload" ] || fail "missing fixture mutation batch: $first"
      apply_mutation_file "$fixture_root" "$payload"
      ;;
    *) fail "unknown fixture operation: $operation" ;;
  esac
}

apply_mutation_file() {
  local fixture_root="$1" mutation_file="$2" operation target first second
  while IFS=$'\t' read -r operation target first second; do
    [ -n "$second" ] || fail "invalid mutation row in ${mutation_file#$ROOT/}"
    apply_mutation "$fixture_root" "$operation" "$target" "$first" "$second"
  done <"$mutation_file"
}

copy_base() {
  local destination="$1"
  mkdir -p "$destination"
  cp -R "$BASE/." "$destination"
  mv "$destination/_bmad-output/project-context.fixture" \
    "$destination/_bmad-output/project-context.md"
  mv "$destination/AGENTS.fixture" "$destination/AGENTS.md"
  cp "$ROOT/AGENTS.md" "$destination/AGENTS.md"
  cp "$ROOT/CLAUDE.md" "$destination/CLAUDE.md"
  for helper in build-pages.sh test-build-pages.sh verify-sample-reports.sh; do
    cp "$ROOT/scripts/$helper" "$destination/scripts/$helper"
  done
  mkdir -p "$destination/.claude"
  cp -R "$destination/fixture-claude/." "$destination/.claude"
  rm -rf "$destination/fixture-claude"
}

make_fixture() {
  local destination="$1" operation="$2" target="$3" first="$4" second="$5"
  copy_base "$destination"
  apply_mutation "$destination" "$operation" "$target" "$first" "$second"
}

matches_selection() {
  [ "$SELECTED_GROUP" = "all" ] || [ "$SELECTED_GROUP" = "$1" ]
}

preflight_fixtures() {
  local group case_name operation target first second expected destination
  while IFS=$'\t' read -r group case_name operation target first second expected; do
    [ -n "$expected" ] || fail "missing expected diagnostic for $case_name"
    destination="$TEST_ROOT/preflight-$case_name"
    make_fixture "$destination" "$operation" "$target" "$first" "$second"
  done <"$MATRIX"
}

track_fixture() {
  git -C "$1" init -q
  git -C "$1" add -f -A
}

verify_valid_base() {
  local fixture_root="$TEST_ROOT/valid" output
  copy_base "$fixture_root"
  track_fixture "$fixture_root"
  if ! output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "production verifier rejected the valid base fixture"
  fi
}

verify_negative_case() {
  local case_name="$1" operation="$2" target="$3" first="$4" second="$5" expected="$6"
  local fixture_root="$TEST_ROOT/case-$case_name" output
  make_fixture "$fixture_root" "$operation" "$target" "$first" "$second"
  track_fixture "$fixture_root"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted negative fixture: $case_name"
  fi
  if ! grep -Fq "$expected" <<<"$output"; then
    printf '%s\n' "$output" >&2
    fail "negative fixture lacked '$expected' diagnostic: $case_name"
  fi
  printf 'PASS %s\n' "$case_name"
}

expect_rejection() {
  local case_name="$1" fixture_root="$2" expected="$3" output
  track_fixture "$fixture_root"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted adversarial guard fixture: $case_name"
  fi
  if ! grep -Fq "$expected" <<<"$output"; then
    printf '%s\n' "$output" >&2
    fail "adversarial guard lacked '$expected' diagnostic: $case_name"
  fi
  printf 'PASS %s\n' "$case_name"
}

expect_exact_rejection() {
  local case_name="$1" fixture_root="$2" expected="$3" output
  track_fixture "$fixture_root"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted exact-diagnostic fixture: $case_name"
  fi
  if [ "$output" != "$expected" ]; then
    printf '%s\n' "$output" >&2
    fail "exact diagnostic mismatch for $case_name"
  fi
  printf 'PASS %s\n' "$case_name"
}

rename_object_field() {
  local file="$1" object_id="$2" field="$3" temporary="$1.mutating"
  awk -v object_id="\"id\": \"$object_id\"" -v field="\"$field\"" '
    index($0, object_id) { in_object = 1 }
    in_object && !renamed && index($0, field) {
      renamed++
      sub(field, "\"" substr(field, 2, length(field) - 2) "_removed\"")
    }
    { print }
    END { if (renamed != 1) exit 1 }
  ' "$file" >"$temporary" || fail "unable to rename $field in $object_id"
  mv "$temporary" "$file"
}

verify_adversarial_guards() {
  local fixture_root ci_path context_path old_ci_hash new_ci_hash

  if matches_selection "context-and-ci-drift"; then
    verify_pages_helper_baseline
    fixture_root="$TEST_ROOT/guard-ci-disabled"
    copy_base "$fixture_root"
    ci_path="$fixture_root/.github/workflows/ci.yml"
    context_path="$fixture_root/_bmad-output/project-context.md"
    old_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$ci_path" \
      "          bash scripts/test-verify-bmad.sh" \
      "          true # bash scripts/test-verify-bmad.sh"
    replace_token "$ci_path" \
      "          bash scripts/verify-bmad.sh" \
      "          true # bash scripts/verify-bmad.sh"
    new_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$context_path" "$old_ci_hash" "$new_ci_hash"
    expect_rejection "ci-disabled-governance-gate" "$fixture_root" "CI additive BMAD governance gate"

    fixture_root="$TEST_ROOT/guard-ci-shell-bypass"
    copy_base "$fixture_root"
    ci_path="$fixture_root/.github/workflows/ci.yml"
    context_path="$fixture_root/_bmad-output/project-context.md"
    old_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$ci_path" \
      "      - name: Verify BMAD governance" \
      "      - shell: bash -c 'true' {0}"
    new_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$context_path" "$old_ci_hash" "$new_ci_hash"
    expect_rejection "ci-shell-bypass" "$fixture_root" "CI additive BMAD governance gate"

    fixture_root="$TEST_ROOT/guard-ci-python-version-bypass"
    copy_base "$fixture_root"
    ci_path="$fixture_root/.github/workflows/ci.yml"
    context_path="$fixture_root/_bmad-output/project-context.md"
    old_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$ci_path" \
      '          python-version: "3.14.8"' \
      '          python-version: "3.14"'
    new_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$context_path" "$old_ci_hash" "$new_ci_hash"
    expect_rejection "ci-python-version-bypass" "$fixture_root" \
      "CI additive BMAD governance gate"

    fixture_root="$TEST_ROOT/guard-ci-python-setup-bypass"
    copy_base "$fixture_root"
    ci_path="$fixture_root/.github/workflows/ci.yml"
    context_path="$fixture_root/_bmad-output/project-context.md"
    old_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$ci_path" \
      '        uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7.0.0' \
      '        run: true # bypassed exact setup-python action'
    new_ci_hash="$(sha256_file "$ci_path")"
    replace_token "$context_path" "$old_ci_hash" "$new_ci_hash"
    expect_rejection "ci-python-setup-bypass" "$fixture_root" \
      "CI additive BMAD governance gate"

    fixture_root="$TEST_ROOT/guard-context-review-metadata"
    copy_base "$fixture_root"
    replace_token "$fixture_root/_bmad-output/project-context.md" \
      "reviewer_identity_type: repository-local-alias" \
      "reviewer_identity_type_removed: repository-local-alias"
    expect_rejection "context-review-metadata" "$fixture_root" "project context review"
    verify_ci_location_guards
    verify_ci_fresh_render_guard
    verify_context_onboarding_guards
  fi

  if matches_selection "launchers-studies-publication"; then
    verify_local_state_ignore_guards
    verify_manifest_and_policy_guards
    verify_protected_study_guards
    verify_sensitive_content_guards
    verify_publication_encoding_guards

    fixture_root="$TEST_ROOT/guard-tracked-codex-state"
    copy_base "$fixture_root"
    mkdir -p "$fixture_root/.agents"
    cp "$FIXTURES/payloads/claude-state.json" "$fixture_root/.agents/settings.local.json"
    expect_rejection "tracked-codex-state" "$fixture_root" "tracked Codex state"

    fixture_root="$TEST_ROOT/guard-tracked-bytecode"
    copy_base "$fixture_root"
    mkdir -p "$fixture_root/_bmad/workflows/__pycache__"
    cp "$FIXTURES/payloads/claude-state.json" \
      "$fixture_root/_bmad/workflows/__pycache__/local.pyc"
    expect_rejection "tracked-bmad-bytecode" "$fixture_root" "tracked BMAD local state"

    fixture_root="$TEST_ROOT/guard-rendered-publication"
    copy_base "$fixture_root"
    mkdir -p "$fixture_root/target/book"
    cp "$FIXTURES/payloads/mdbook-publication.txt" "$fixture_root/target/book/index.html"
    expect_rejection "rendered-bmad-publication" "$fixture_root" "rendered publication"

    fixture_root="$TEST_ROOT/guard-rendered-pages-publication"
    copy_base "$fixture_root"
    mkdir -p "$fixture_root/target/pages"
    cp "$FIXTURES/payloads/mdbook-publication.txt" "$fixture_root/target/pages/index.html"
    expect_rejection "rendered-pages-bmad-publication" "$fixture_root" "rendered publication"
  fi

  if matches_selection "authority-and-artifacts"; then
    verify_authority_schema_guards
    verify_artifact_governance_guards
    verify_urgent_incident_guards

    fixture_root="$TEST_ROOT/guard-authority-required-field"
    copy_base "$fixture_root"
    rename_object_field "$fixture_root/_bmad-output/authorities.yaml" "security-1" "verified_human"
    expect_rejection "authority-required-field" "$fixture_root" "authority identity record"

    fixture_root="$TEST_ROOT/guard-artifact-required-field"
    copy_base "$fixture_root"
    rename_object_field \
      "$fixture_root/_bmad-output/index.yaml" "urgent-security-incident-001" "archive_evidence"
    expect_rejection "artifact-required-field" "$fixture_root" "artifact record schema"
  fi

  if matches_selection "lock-and-paths"; then
    verify_lock_schema_guards

    fixture_root="$TEST_ROOT/guard-installer-integrity"
    copy_base "$fixture_root"
    replace_token "$fixture_root/bmad.lock.json" \
      "1e19d145b9e45fa4340655506958b4c680d32e72fa1b5faa2b70c7770ea84102ccd727f690b2eb6e3b73647322eef516c6cc9e16c26d9bc68983edd61131a1a3" \
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    expect_rejection "installer-integrity-cross-field" "$fixture_root" "installer integrity"

    fixture_root="$TEST_ROOT/guard-generation-environment"
    copy_base "$fixture_root"
    replace_token "$fixture_root/bmad.lock.json" \
      "fresh-disposable-baseline-clone" "working-checkout"
    expect_rejection "generation-environment-cross-field" "$fixture_root" "generation environment"

    fixture_root="$TEST_ROOT/guard-repository-promotion"
    copy_base "$fixture_root"
    replace_token "$fixture_root/bmad.lock.json" \
      '"installer_executed_in_repository": false' '"installer_executed_in_repository": true'
    expect_rejection "repository-promotion-cross-field" "$fixture_root" "repository promotion"

    fixture_root="$TEST_ROOT/guard-human-owned-file-record"
    copy_base "$fixture_root"
    replace_token "$fixture_root/bmad.lock.json" \
      '"ownership": "human-owned-team-override"' '"ownership": "generated"'
    expect_rejection "human-owned-file-record" "$fixture_root" "human-owned file inventory"

    fixture_root="$TEST_ROOT/guard-human-owned-root-record"
    copy_base "$fixture_root"
    replace_token "$fixture_root/bmad.lock.json" \
      '"human_owned_output_roots": [' '"human_owned_output_roots": ["malformed",'
    expect_rejection "human-owned-root-record" "$fixture_root" "human-owned root inventory"

    fixture_root="$TEST_ROOT/guard-missing-lock-diagnostic"
    copy_base "$fixture_root"
    rm "$fixture_root/bmad.lock.json"
    expect_exact_rejection "missing-lock-stable-diagnostic" "$fixture_root" \
      "BMAD verification error: missing bmad.lock.json"
  fi
}

verify_deadline_timezone() {
  matches_selection "authority-and-artifacts" || return 0
  if ! PYTHONDONTWRITEBYTECODE=1 python3 - "$ROOT" <<'PY'
import sys

sys.path.insert(0, sys.argv[1] + "/scripts")
from verify_bmad import retrospective_deadline

actual = retrospective_deadline("2026-08-30T23:30:00Z")
expected = "2026-09-07"
if actual != expected:
    raise SystemExit(f"expected Europe/London deadline {expected}, got {actual}")
PY
  then
    fail "urgent-security deadline is not calculated in Europe/London"
  fi
  printf 'PASS urgent-security-europe-london-deadline\n'
}

run_selected_cases() {
  local group case_name operation target first second expected
  while IFS=$'\t' read -r group case_name operation target first second expected; do
    matches_selection "$group" || continue
    verify_negative_case "$case_name" "$operation" "$target" "$first" "$second" "$expected"
  done <"$MATRIX"
}

validate_matrix
verify_fixture_context_isolation
preflight_fixtures
if [ ! -f "$VERIFIER" ]; then
  printf 'ERROR: missing production verifier: scripts/verify-bmad.sh\n' >&2
  exit 1
fi
verify_runtime_selection
verify_read_only_python_runtime
verify_durable_evidence_state
verify_valid_base
run_selected_cases
verify_adversarial_guards
verify_deadline_timezone
printf 'BMAD governance fixture matrix passed: %s\n' "$SELECTED_GROUP"
