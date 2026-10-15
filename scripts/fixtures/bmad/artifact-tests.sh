#!/usr/bin/env bash

artifact_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/index.yaml" "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

artifact_batch_guard() {
  local case_name="$1" payload="$2" expected="$3" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  apply_mutation_file "$fixture_root" "$FIXTURES/payloads/$payload"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

archived_supersession_guard() {
  local case_name="$1" old="$2" new="$3" expected="$4" fixture_root
  fixture_root="$TEST_ROOT/guard-$case_name"
  copy_base "$fixture_root"
  apply_mutation_file "$fixture_root" \
    "$FIXTURES/payloads/artifact-archived-supersession-valid.tsv"
  replace_token "$fixture_root/_bmad-output/index.yaml" "$old" "$new"
  expect_rejection "$case_name" "$fixture_root" "$expected"
}

verify_artifact_governance_guards() {
  local fixture_root readme

  artifact_guard \
    "artifact-index-ownership-missing" \
    '"ownership": "human-owned"' '"ownership_removed": "human-owned"' \
    "artifact index canonical policy"
  artifact_guard \
    "artifact-index-unexpected-field" \
    '"schema_version": 1,' '"schema_version": 1, "unexpected_policy": true,' \
    "artifact index schema fields"
  artifact_guard \
    "artifact-index-revision-bool" \
    '"index_revision": 1' '"index_revision": true' \
    "artifact index revision"
  artifact_guard \
    "artifact-record-unexpected-field" \
    '"id": "plan-001",' '"id": "plan-001", "unexpected_field": null,' \
    "artifact record schema unexpected field"

  artifact_guard \
    "artifact-canonical-roots-required" \
    '"canonical_roots": [' '"canonical_roots_removed": [' \
    "artifact canonical roots"
  artifact_guard \
    "artifact-authority-registry-path" \
    '"authority_registry": "_bmad-output/authorities.yaml"' \
    '"authority_registry": "_bmad-output/other-authorities.yaml"' \
    "artifact authority registry path"
  artifact_guard \
    "artifact-raw-path-canonical" \
    '"path": "_bmad-output/planning-artifacts/plan-001.md"' \
    '"path": "_bmad-output/planning-artifacts/./plan-001.md"' \
    "raw artifact path is not canonical"
  artifact_guard \
    "artifact-evidence-raw-path-canonical" \
    '"approval_evidence": "_bmad-output/governance/plan-approval.md"' \
    '"approval_evidence": "_bmad-output//governance/plan-approval.md"' \
    "raw artifact approval evidence"
  artifact_guard \
    "artifact-independence-fields-null" \
    '"independence_exception_approver":null' \
    '"independence_exception_approver":"don-1"' \
    "artifact independence exception fields must be null"

  fixture_root="$TEST_ROOT/guard-artifact-unindexed-markdown"
  copy_base "$fixture_root"
  cp "$FIXTURES/payloads/mdbook-publication.txt" \
    "$fixture_root/_bmad-output/planning-artifacts/unindexed.md"
  expect_rejection \
    "artifact-unindexed-markdown" "$fixture_root" "unindexed canonical artifact"

  fixture_root="$TEST_ROOT/guard-artifact-nested-readme"
  copy_base "$fixture_root"
  mkdir -p "$fixture_root/_bmad-output/planning-artifacts/nested"
  cp "$FIXTURES/payloads/mdbook-publication.txt" \
    "$fixture_root/_bmad-output/planning-artifacts/nested/README.md"
  expect_rejection \
    "artifact-nested-readme" "$fixture_root" "unindexed canonical artifact"

  artifact_batch_guard \
    "artifact-parent-cycle" "artifact-parent-cycle.tsv" "artifact parent cycle"
  artifact_batch_guard \
    "artifact-epic-parent-type" "artifact-epic-parent.tsv" "artifact parent type"
  artifact_batch_guard \
    "artifact-effective-child-archived-parent" "artifact-archived-parent.tsv" \
    "effective artifact has archived parent"
  artifact_batch_guard \
    "artifact-archive-chronology" "artifact-archive-chronology.tsv" \
    "artifact archive chronology"
  artifact_batch_guard \
    "artifact-supersession-cycle" "artifact-supersession-cycle.tsv" \
    "artifact supersession cycle"
  artifact_batch_guard \
    "artifact-supersession-chronology" "artifact-supersession-chronology.tsv" \
    "artifact supersession chronology"
  archived_supersession_guard \
    "artifact-archived-supersession-partial" \
    '"supersession_authorizer":"maintainer-1"' \
    '"supersession_authorizer":null' \
    "archived artifact supersession fields are incomplete"
  archived_supersession_guard \
    "artifact-archived-supersession-ghost" \
    '"superseded_by":"plan-draft-002"' '"superseded_by":"ghost-plan"' \
    "artifact supersession links are not bidirectional"
  archived_supersession_guard \
    "artifact-archived-supersession-missing-evidence" \
    '"supersession_evidence":"_bmad-output/governance/plan-approval.md"' \
    '"supersession_evidence":"_bmad-output/governance/missing.md"' \
    "missing supersession evidence for plan-001"
  archived_supersession_guard \
    "artifact-archived-supersession-chronology" \
    '"archived_at":"2026-08-14T13:00:00Z"' \
    '"archived_at":"2026-08-14T11:30:00Z"' \
    "artifact archive chronology is invalid"

  for readme in \
    "$ROOT/_bmad-output/planning-artifacts/README.md" \
    "$ROOT/_bmad-output/implementation-artifacts/README.md"
  do
    grep -Fq 'workflow frontmatter `type` and `status`' "$readme" || \
      fail "artifact README does not distinguish workflow frontmatter: ${readme#$ROOT/}"
    grep -Fq '`in-review`' "$readme" || \
      fail "artifact README does not document in-review operation: ${readme#$ROOT/}"
  done
  printf 'PASS artifact-workflow-index-domain-distinction\n'

  fixture_root="$TEST_ROOT/guard-artifact-container-type"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/index.yaml" \
    '"artifact_type": "urgent-security-incident"' \
    '"artifact_type": ["urgent-security-incident"]'
  expect_exact_rejection "artifact-container-type" "$fixture_root" \
    "BMAD verification error: artifact record field type is invalid: urgent-security-incident-001"

  fixture_root="$TEST_ROOT/guard-artifact-nested-parent"
  copy_base "$fixture_root"
  replace_token "$fixture_root/_bmad-output/index.yaml" \
    '"parent": "plan-001"' '"parent": [["plan-001"]]'
  expect_exact_rejection "artifact-nested-parent" "$fixture_root" \
    "BMAD verification error: artifact record field type is invalid: story-001"
}
