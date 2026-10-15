#!/usr/bin/env bash

lock_guard() {
  local case_name="$1" operation="$2" path="$3" value="$4" expected="$5"
  local fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  if [ "$operation" = "delete" ]; then
    python3 "$FIXTURES/mutate-json.py" \
      "$fixture_root/bmad.lock.json" delete "$path"
  else
    python3 "$FIXTURES/mutate-json.py" \
      "$fixture_root/bmad.lock.json" set "$path" "$value"
  fi
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_lock_schema_guards() {
  lock_guard "lock-schema-bool" set schema_version true "BMAD lock schema"
  lock_guard "lock-version-policy" delete version_policy.automatic_updates null \
    "version policy"
  lock_guard "lock-installer-entrypoint" set installer.entry_point '"mutable.js"' \
    "installer entry point"
  lock_guard "lock-installer-lifecycle" delete installer.lifecycle.postinstall null \
    "installer lifecycle"
  lock_guard "lock-output-root" set scope.output_root '"generated-output"' \
    "output root"
  lock_guard "lock-platform" delete platform.os_version null \
    "platform metadata"

  lock_guard "dependency-direct-count-bool" set dependencies.direct_count true \
    "dependency count"
  lock_guard "dependency-node-schema" delete \
    dependencies.nodes.0.optional_dependencies null "dependency node schema"
  lock_guard "dependency-edge-schema" set dependencies.nodes.0.dependencies '[]' \
    "dependency edge schema"
  lock_guard "dependency-sri" set dependencies.nodes.0.integrity '"sha512-invalid"' \
    "dependency integrity"
  lock_guard "dependency-digest" set dependencies.canonical_payload_sha512 \
    '"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
    "dependency graph digest"
  lock_guard "dependency-resolution-audit" delete dependencies.resolution_problems null \
    "dependency resolution audit"
  lock_guard "dependency-install-script-audit" set dependencies.install_script_packages \
    '["synthetic-package"]' "install scripts"
  lock_guard "dependency-audit-field" delete dependency_audit.repository_mutated null \
    "dependency audit"

  lock_guard "generation-environment-field" delete generation.environment.os_version null \
    "generation environment"
  lock_guard "generation-raw-inventory" delete \
    generation.raw_installer_inventory.framework_files null "raw installer inventory"
  lock_guard "generation-safety-audit" delete generation.safety_audit.credential_hit_count \
    null "generation safety audit"
  lock_guard "generation-reproduction-field" delete \
    generation.reproducibility.second_raw_promotion_sha256 null \
    "generation reproducibility proof"
  lock_guard "promotion-human-owned-field" delete repository_promotion.human_owned_files \
    null "repository promotion proof"
  lock_guard "human-owned-record-field" delete \
    generation.inventory.human_owned_files.0.ownership null "human-owned file inventory"
  lock_guard "human-owned-root-overlap" set \
    generation.inventory.human_owned_output_roots.0.path '"_bmad"' \
    "generated and human-owned path overlap"
}
