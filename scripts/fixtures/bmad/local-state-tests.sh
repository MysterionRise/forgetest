#!/usr/bin/env bash

expect_visible_local_state_rejection() {
  local case_name="$1" target="$2" fixture_root output rule
  shift 2
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  track_fixture "$fixture_root"
  for rule in "$@"; do
    printf '%s\n' "$rule" >>"$fixture_root/.gitignore"
  done
  mkdir -p "$(dirname "$fixture_root/$target")"
  cp "$FIXTURES/payloads/claude-state.json" "$fixture_root/$target"
  if output="$(bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted visible untracked local state: $case_name"
  fi
  if ! grep -Fq "local Claude, Codex, or BMAD state is not ignored: $target" \
      <<<"$output"; then
    printf '%s\n' "$output" >&2
    fail "visible local-state guard lacked stable diagnostic: $case_name"
  fi
  printf 'PASS %s\n' "$case_name"
}

verify_local_state_ignore_guards() {
  local fixture_root launcher output

  expect_visible_local_state_rejection \
    "negated-codex-local-state" ".agents/settings.local.json" \
    "!/.agents/settings.local.json"
  expect_visible_local_state_rejection \
    "negated-claude-local-state" ".claude/settings.local.json" \
    "!/.claude/settings.local.json"
  expect_visible_local_state_rejection \
    "negated-bmad-user-state" "_bmad/config.user.toml" \
    "!/_bmad/config.user.toml"
  expect_visible_local_state_rejection \
    "negated-bmad-nested-user-state" "_bmad/workflows/local.user.toml" \
    "!/_bmad/workflows/local.user.toml"
  expect_visible_local_state_rejection \
    "negated-bmad-cache-state" "_bmad/workflows/__pycache__/local.pyc" \
    "!/_bmad/workflows/__pycache__/" \
    "!/_bmad/workflows/__pycache__/local.pyc"

  fixture_root="$TEST_ROOT/guard-bmad-launchers-visible"
  copy_base "$fixture_root"
  track_fixture "$fixture_root"
  for launcher in \
    ".agents/skills/bmad-visible/SKILL.md" \
    ".claude/skills/bmad-visible/SKILL.md"
  do
    mkdir -p "$(dirname "$fixture_root/$launcher")"
    cp "$FIXTURES/payloads/claude-state.json" "$fixture_root/$launcher"
    output="$(git -C "$fixture_root" ls-files --others --exclude-standard -- "$launcher")"
    [ "$output" = "$launcher" ] || \
      fail "allowed BMAD launcher is hidden by ignore rules: $launcher"
  done
  printf 'PASS allowed-bmad-launchers-visible\n'
}
