#!/usr/bin/env bash

write_runtime_stub() {
  local path="$1" mode="$2"
  case "$mode" in
    preferred)
      printf '%s\n' \
        '#!/usr/bin/env bash' \
        'if [ "${1:-}" = "-c" ]; then exit 0; fi' \
        'printf "preferred-python\\n"'
      ;;
    unexpected)
      printf '%s\n' \
        '#!/usr/bin/env bash' \
        'exit 97'
      ;;
    fallback)
      printf '%s\n' \
        '#!/usr/bin/env bash' \
        'if [ "${1:-}" = "-c" ]; then' \
        '  : >"${BMAD_TEST_PROBE:?}"' \
        '  exit 0' \
        'fi' \
        'printf "fallback-python\\n"'
      ;;
    unsupported)
      printf '%s\n' \
        '#!/usr/bin/env bash' \
        'if [ "${1:-}" = "-c" ]; then exit 1; fi' \
        'printf "unsupported-python-was-executed\\n"'
      ;;
    *) fail "unknown runtime stub mode: $mode" ;;
  esac >"$path"
  chmod +x "$path"
}

verify_runtime_selection() {
  [ "$SELECTED_GROUP" = "all" ] || return 0
  local runtime_root="$TEST_ROOT/runtime-selection" runtime_bin output status probe
  local helper missing helper_root expected

  runtime_bin="$runtime_root/preferred"
  mkdir -p "$runtime_bin"
  write_runtime_stub "$runtime_bin/python3.10" preferred
  write_runtime_stub "$runtime_bin/python3" unexpected
  if ! output="$(PATH="$runtime_bin:/usr/bin:/bin" bash "$VERIFIER" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "runtime selector did not prefer python3.10"
  fi
  [ "$output" = "preferred-python" ] || \
    fail "runtime selector did not execute preferred python3.10"
  printf 'PASS runtime-prefers-python3.10\n'

  runtime_bin="$runtime_root/fallback"
  probe="$runtime_root/fallback-probed"
  mkdir -p "$runtime_bin"
  write_runtime_stub "$runtime_bin/python3" fallback
  if ! output="$(BMAD_TEST_PROBE="$probe" PATH="$runtime_bin:/usr/bin:/bin" \
      bash "$VERIFIER" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "runtime selector rejected supported python3 fallback"
  fi
  [ "$output" = "fallback-python" ] || \
    fail "runtime selector did not execute supported python3 fallback"
  [ -f "$probe" ] || fail "runtime selector did not version-check python3 fallback"
  printf 'PASS runtime-verifies-python3-fallback\n'

  runtime_bin="$runtime_root/unsupported"
  mkdir -p "$runtime_bin"
  write_runtime_stub "$runtime_bin/python3" unsupported
  set +e
  output="$(PATH="$runtime_bin:/usr/bin:/bin" bash "$VERIFIER" 2>&1)"
  status=$?
  set -e
  [ "$status" -ne 0 ] || fail "runtime selector accepted unsupported python3"
  [ "$output" = "BMAD verification error: Python 3.10 or newer is required" ] || \
    fail "runtime selector emitted an unstable Python requirement diagnostic"
  printf 'PASS runtime-rejects-unsupported-python\n'

  for missing in \
    verify_bmad.py \
    bmad_verify_artifact_policy.py \
    bmad_verify_authority.py \
    bmad_verify_common.py \
    bmad_verify_context.py \
    bmad_verify_governance.py \
    bmad_verify_lock.py \
    bmad_verify_policy.py \
    bmad_verify_promotion.py \
    bmad_verify_provenance.py \
    bmad_verify_sensitive.py \
    bmad_verify_studies.py \
    bmad_verify_surfaces.py
  do
    helper_root="$runtime_root/missing-$missing"
    mkdir -p "$helper_root"
    cp "$VERIFIER" "$helper_root/verify-bmad.sh"
    for helper in \
      verify_bmad.py \
      bmad_verify_artifact_policy.py \
      bmad_verify_authority.py \
      bmad_verify_common.py \
      bmad_verify_context.py \
      bmad_verify_governance.py \
      bmad_verify_lock.py \
      bmad_verify_policy.py \
      bmad_verify_promotion.py \
      bmad_verify_provenance.py \
      bmad_verify_sensitive.py \
      bmad_verify_studies.py \
      bmad_verify_surfaces.py
    do
      cp "$ROOT/scripts/$helper" "$helper_root/$helper"
    done
    rm "$helper_root/$missing"
    set +e
    output="$(bash "$helper_root/verify-bmad.sh" 2>&1)"
    status=$?
    set -e
    expected="BMAD verification error: missing verifier helper: scripts/$missing"
    [ "$status" -ne 0 ] || fail "runtime selector accepted missing helper: $missing"
    [ "$output" = "$expected" ] || \
      fail "runtime selector emitted an unstable missing-helper diagnostic: $missing"
  done
  printf 'PASS runtime-rejects-missing-production-helper\n'
}

verify_fixture_context_isolation() {
  [ "$SELECTED_GROUP" = "all" ] || return 0
  local discovered nested fixture_root

  discovered="$(find "$ROOT" -type f -name project-context.md -print)"
  [ "$discovered" = "$ROOT/_bmad-output/project-context.md" ] ||
    fail "fixture project context is visible to default recursive discovery"

  nested="$(find "$FIXTURES" -type f -name AGENTS.md -print -quit)"
  [ -z "$nested" ] || fail "fixture AGENTS.md applies inside the live checkout"

  fixture_root="$TEST_ROOT/fixture-context-isolation"
  copy_base "$fixture_root"
  [ -f "$fixture_root/_bmad-output/project-context.md" ] ||
    fail "copy_base did not materialize canonical project context"
  [ -f "$fixture_root/AGENTS.md" ] ||
    fail "copy_base did not materialize canonical repository instructions"
  printf 'PASS fixture-context-isolation\n'
}

verify_read_only_python_runtime() {
  [ "$SELECTED_GROUP" = "all" ] || return 0
  local verifier_root="$TEST_ROOT/read-only-verifier" fixture_root output residue

  mkdir -p "$verifier_root"
  cp "$VERIFIER" "$verifier_root/verify-bmad.sh"
  cp "$ROOT"/scripts/bmad_verify_*.py "$ROOT/scripts/verify_bmad.py" "$verifier_root/"
  fixture_root="$TEST_ROOT/read-only-verifier-fixture"
  copy_base "$fixture_root"
  track_fixture "$fixture_root"
  if ! output="$(bash "$verifier_root/verify-bmad.sh" --root "$fixture_root" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "read-only verifier regression could not execute clean temp copy"
  fi
  residue="$(find "$verifier_root" \( -type d -name __pycache__ -o -type f -name '*.pyc' \) \
    -print -quit)"
  [ -z "$residue" ] || fail "public verifier created Python bytecode state"
  printf 'PASS runtime-does-not-write-python-bytecode\n'
}

verify_durable_evidence_state() {
  [ "$SELECTED_GROUP" = "all" ] || return 0
  local evidence="$ROOT/docs/gangsta/bmad-repository-setup/evidence/2026-08-14-bmad-package-audit.md"
  local specification="$ROOT/_bmad-output/implementation-artifacts/spec-bmad-repository-setup.md"

  grep -Fxq "status: blocked" "$evidence" ||
    fail "package-audit evidence is not blocked on the live RustSec failure"
  grep -Fxq "status: 'in-review'" "$specification" ||
    fail "BMAD implementation specification left in-review state"
  grep -Fq 'RUSTSEC-2026-0258' "$evidence" ||
    fail "package-audit evidence lost the h2 advisory"
  grep -Fq '`chacha20` 0.10.1 is yanked' "$evidence" ||
    fail "package-audit evidence lost the chacha20 warning"
  printf 'PASS durable-evidence-remains-blocked\n'
}
