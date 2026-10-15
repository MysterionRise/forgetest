#!/usr/bin/env bash

refresh_context_identity() {
  local fixture_root="$1" relative="$2" old_hash="$3"
  replace_token "$fixture_root/_bmad-output/project-context.md" \
    "$old_hash" "$(sha256_file "$fixture_root/$relative")"
}

verify_pages_helper_baseline() {
  local expected="145f33a8cc418fdf85c63eecf0bb0e54ce34e1c8f2c1b2236f2bd76268301b92"
  [ "$(sha256_file "$ROOT/scripts/test-build-pages.sh")" = "$expected" ] ||
    fail "scripts/test-build-pages.sh differs from the locked baseline bytes"
  printf 'PASS pages-helper-baseline-bytes\n'
}

ci_location_guard() {
  local mode="$1" expected="$2" fixture_root ci_path old_hash
  fixture_root="$TEST_ROOT/guard-ci-$mode"
  copy_base "$fixture_root"
  ci_path="$fixture_root/.github/workflows/ci.yml"
  old_hash="$(sha256_file "$ci_path")"
  python3 "$FIXTURES/mutate-ci-gate.py" "$ci_path" "$mode"
  refresh_context_identity "$fixture_root" ".github/workflows/ci.yml" "$old_hash"
  expect_rejection "ci-$mode-gate" "$fixture_root" "$expected"
}

verify_ci_location_guards() {
  ci_location_guard "moved" "CI additive BMAD governance gate is not at the approved quality offset"
  ci_location_guard "root" "CI additive BMAD gate markers are invalid"
  ci_location_guard "nested" "CI baseline drift"
  ci_location_guard "reversed" "CI additive BMAD gate marker order"
}

verify_ci_fresh_render_guard() {
  local fixture_root="$TEST_ROOT/guard-ci-fresh-render-bypass" ci_path old_hash
  fixture_root="$TEST_ROOT/guard-ci-fresh-render-bypass"
  copy_base "$fixture_root"
  ci_path="$fixture_root/.github/workflows/ci.yml"
  old_hash="$(sha256_file "$ci_path")"
  replace_token "$ci_path" \
    "        run: bash scripts/build-pages.sh" \
    "        run: true # bypassed fresh rendered Pages build"
  refresh_context_identity "$fixture_root" ".github/workflows/ci.yml" "$old_hash"
  expect_rejection "ci-fresh-render-bypass" "$fixture_root" \
    "CI additive BMAD governance gate"
}

verify_context_onboarding_guards() {
  local fixture_root
  fixture_root="$TEST_ROOT/guard-context-approved-review"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/project-context.md" \
    "2026-08-29T12:15:33Z" "2026-08-14T12:00:00Z"
  expect_rejection "context-approved-review" "$fixture_root" \
    "project context approved review metadata"

  fixture_root="$TEST_ROOT/guard-readme-runtime-onboarding"
  copy_base "$fixture_root"
  replace_token "$fixture_root/README.md" \
    "BMAD launcher rendering requires" "BMAD launcher rendering formerly required"
  expect_rejection "readme-runtime-onboarding" "$fixture_root" \
    "README BMAD runtime onboarding"

  fixture_root="$TEST_ROOT/guard-context-runtime-onboarding"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/project-context.md" \
    "BMAD launcher rendering requires" "BMAD launcher rendering formerly required"
  expect_rejection "context-runtime-onboarding" "$fixture_root" \
    "project context BMAD runtime onboarding"

  fixture_root="$TEST_ROOT/guard-render-smoke-missing"
  copy_base "$fixture_root"
  rm "$fixture_root/scripts/test-bmad-render.sh"
  expect_rejection "render-smoke-missing" "$fixture_root" \
    "missing BMAD launcher render smoke check"

  fixture_root="$TEST_ROOT/guard-render-smoke-bypass"
  copy_base "$fixture_root"
  replace_token "$fixture_root/scripts/test-bmad-render.sh" \
    "uv run --no-cache" "uv run"
  expect_rejection "render-smoke-bypass" "$fixture_root" \
    "BMAD launcher render smoke contract"

  fixture_root="$TEST_ROOT/guard-context-ci-inventory-semantics"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/project-context.md" \
    "      name: fixture quality" "      name: renamed fixture quality"
  expect_rejection "context-ci-inventory-semantics" "$fixture_root" \
    "project context CI proof contract diverges from lock"
}

publication_guard() {
  local case_name="$1" target="$2" content="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  mkdir -p "$(dirname "$fixture_root/$target")"
  printf '%s\n' "$content" >"$fixture_root/$target"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_book_section_guard() {
  local fixture_root="$TEST_ROOT/guard-book-section-source" target old_hash temporary
  copy_base "$fixture_root"
  target="$fixture_root/docs/book.toml"
  old_hash="$(sha256_file "$target")"
  temporary="$target.mutating"
  awk '
    BEGIN { print "src = \"src\"" }
    /^\[book\]$/ { in_book = 1 }
    in_book && /^src = "src"$/ { print "src = \"_bmad-output\""; changed++; next }
    { print }
    END { if (changed != 1) exit 1 }
  ' "$target" >"$temporary" || fail "unable to mutate fixture [book] source"
  mv "$temporary" "$target"
  refresh_context_identity "$fixture_root" "docs/book.toml" "$old_hash"
  expect_rejection "book-section-source" "$fixture_root" "BMAD publication source root is forbidden"
}

verify_required_rendered_guards() {
  local fixture_root output
  fixture_root="$TEST_ROOT/guard-required-rendered-missing"
  copy_base "$fixture_root"
  track_fixture "$fixture_root"
  if output="$(BMAD_RENDERED_OUTPUT=target/pages bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted absent required rendered output"
  fi
  grep -Fq "required rendered publication output is missing: target/pages" <<<"$output" || {
    printf '%s\n' "$output" >&2
    fail "required rendered-output guard lacked stable diagnostic"
  }
  printf 'PASS required-rendered-output-missing\n'

  fixture_root="$TEST_ROOT/guard-required-rendered-safe"
  copy_base "$fixture_root"
  mkdir -p "$fixture_root/target/pages"
  printf '<p>safe synthetic rendered page</p>\n' >"$fixture_root/target/pages/index.html"
  track_fixture "$fixture_root"
  if ! output="$(BMAD_RENDERED_OUTPUT=target/pages bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "production verifier rejected safe required rendered output"
  fi
  printf 'PASS required-rendered-output-safe\n'
}

rendered_output_guard() {
  local case_name="$1" rendered_value="$2" expected="$3" fixture_root output
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  mkdir -p "$fixture_root/target/pages"
  printf '<p>safe synthetic rendered page</p>\n' >"$fixture_root/target/pages/index.html"
  track_fixture "$fixture_root"
  if output="$(BMAD_RENDERED_OUTPUT="$rendered_value" \
      bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted unsafe rendered output: $case_name"
  fi
  grep -Fq "$expected" <<<"$output" || {
    printf '%s\n' "$output" >&2
    fail "rendered-output guard lacked stable diagnostic: $case_name"
  }
  printf 'PASS %s\n' "$case_name"
}

verify_rendered_output_path_guards() {
  local fixture_root output
  rendered_output_guard "rendered-output-noncanonical" "target//pages" \
    "non-canonical rendered publication path"

  fixture_root="$TEST_ROOT/guard-rendered-output-symlink-root"
  copy_base "$fixture_root"
  mkdir -p "$fixture_root/target/actual-pages"
  printf '<p>safe</p>\n' >"$fixture_root/target/actual-pages/index.html"
  ln -s actual-pages "$fixture_root/target/pages"
  track_fixture "$fixture_root"
  if output="$(BMAD_RENDERED_OUTPUT=target/pages \
      bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted symlink rendered-output root"
  fi
  grep -Fq "symlink rendered publication target is forbidden: target/pages" \
    <<<"$output" || fail "rendered-output root symlink lacked stable diagnostic"
  printf 'PASS rendered-output-symlink-root\n'

  fixture_root="$TEST_ROOT/guard-rendered-output-symlink-descendant"
  copy_base "$fixture_root"
  mkdir -p "$fixture_root/target/pages"
  printf '<p>safe</p>\n' >"$fixture_root/target/pages/index.html"
  ln -s index.html "$fixture_root/target/pages/alias.html"
  track_fixture "$fixture_root"
  if output="$(BMAD_RENDERED_OUTPUT=target/pages \
      bash "$VERIFIER" --root "$fixture_root" 2>&1)"; then
    fail "production verifier accepted symlink rendered-output descendant"
  fi
  grep -Fq "symlink rendered publication target is forbidden: target/pages/alias.html" \
    <<<"$output" || fail "rendered-output descendant symlink lacked stable diagnostic"
  printf 'PASS rendered-output-symlink-descendant\n'
}

verify_delegated_publication_helper_guard() {
  local fixture_root="$TEST_ROOT/guard-delegated-publication-helper"
  copy_base "$fixture_root"
  printf '\nbash "$ROOT/scripts/delegated-publication.sh"\n' \
    >>"$fixture_root/scripts/build-pages.sh"
  printf 'cp -R %%5Fbmad-output target/pages/private\n' \
    >"$fixture_root/scripts/delegated-publication.sh"
  expect_rejection "delegated-publication-helper" "$fixture_root" \
    "internal BMAD publication copy or workflow rule detected: scripts/delegated-publication.sh"
}

verify_script_dir_publication_helper_guard() {
  local fixture_root="$TEST_ROOT/guard-script-dir-publication-helper"
  copy_base "$fixture_root"
  printf '\nbash "$SCRIPT_DIR/delegated-publication.sh"\n' \
    >>"$fixture_root/scripts/build-pages.sh"
  printf 'cp -R %%5Fbmad-output target/pages/private\n' \
    >"$fixture_root/scripts/delegated-publication.sh"
  expect_rejection "script-dir-publication-helper" "$fixture_root" \
    "internal BMAD publication copy or workflow rule detected: scripts/delegated-publication.sh"
}

verify_publication_helper_traversal_guard() {
  local fixture_root="$TEST_ROOT/guard-publication-helper-traversal"
  copy_base "$fixture_root"
  printf '\nbash "$ROOT/scripts/../../outside.sh"\n' \
    >>"$fixture_root/scripts/build-pages.sh"
  printf '# safe synthetic outside helper\n' >"$TEST_ROOT/outside.sh"
  expect_rejection "publication-helper-traversal" "$fixture_root" \
    "traversal in publication build helper path is forbidden: scripts/../../outside.sh"
}

verify_publication_encoding_guards() {
  verify_book_section_guard
  publication_guard "encoded-doc-link" "docs/src/encoded.md" \
    '[internal](https%3A%2F%2Fexample.invalid%2F%5Fbmad-output%2Fprivate)' \
    "internal BMAD publication reference"
  publication_guard "encoded-copy-rule" "scripts/build-pages.sh" \
    'cp -R %5Fbmad-output target/pages/private' \
    "internal BMAD publication copy or workflow rule"
  publication_guard "encoded-workflow-rule" ".github/workflows/pages-copy.yml" \
    'name: Pages copy; run: cp -R %5Fbmad target/pages/private' \
    "internal BMAD publication copy or workflow rule"
  publication_guard "encoded-rendered-content" "target/pages/index.html" \
    '<a href="%5Fbmad-output/private">internal</a>' \
    "internal BMAD rendered publication content"
  verify_required_rendered_guards
  verify_rendered_output_path_guards
  verify_delegated_publication_helper_guard
  verify_script_dir_publication_helper_guard
  verify_publication_helper_traversal_guard
}
