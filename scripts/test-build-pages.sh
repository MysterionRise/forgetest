#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="$(mktemp -d "${TMPDIR:-/tmp}/forgetest-pages.XXXXXX")"
trap 'rm -rf "$OUTPUT"' EXIT

bash "$ROOT/scripts/build-pages.sh" "$OUTPUT"

test -f "$OUTPUT/index.html"
test -f "$OUTPUT/evidence.html"
test -f "$OUTPUT/.nojekyll"
test -f "$OUTPUT/assets/repository-report.png"
test -f "$OUTPUT/reports/repository-local/report.html"
test -f "$OUTPUT/reports/repository-local/report.json"
test -f "$OUTPUT/reports/repository-local/artifact-manifest.json"
test -f "$OUTPUT/reports/repository-docker/report.html"
test -f "$OUTPUT/reports/snippet-local/report-2026-07-27T155019.html"
test -f "$OUTPUT/reports/docker/report-2026-07-27T155934.html"

grep -Fq "deterministic offline evidence" "$OUTPUT/index.html"
grep -Fq "deterministic offline evidence" "$OUTPUT/evidence.html"

cp -R "$ROOT/examples/reports" "$OUTPUT/leaky-reports"
printf 'workspace=/Users/example/private/forgetest-trial-test/verifier-workspace\n' \
  >"$OUTPUT/leaky-reports/repository-local/leak.txt"
if bash "$ROOT/scripts/verify-sample-reports.sh" "$OUTPUT/leaky-reports" \
  >"$OUTPUT/leak-check.log" 2>&1; then
  echo "sample-report verification accepted an absolute private path" >&2
  exit 1
fi
grep -Fq "publication-safe path scan failed" "$OUTPUT/leak-check.log"

if bash "$ROOT/scripts/build-pages.sh" "$OUTPUT/.." >/dev/null 2>&1; then
  echo "Pages builder accepted the temporary-directory root" >&2
  exit 1
fi

echo "Pages artifact contract verified"
