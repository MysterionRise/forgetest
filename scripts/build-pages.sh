#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ROOT="$ROOT/target"
TEMP_ROOT="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
mkdir -p "$TARGET_ROOT"

if [ "$#" -eq 0 ]; then
  OUTPUT="$TARGET_ROOT/pages"
else
  if [ ! -d "$1" ]; then
    echo "custom Pages output must be an existing directory: $1" >&2
    exit 1
  fi
  OUTPUT="$(cd "$1" && pwd -P)"
fi

case "$OUTPUT" in
  "$TARGET_ROOT"/* | "$TEMP_ROOT"/*)
    ;;
  *)
    echo "refusing to replace Pages output outside target or a temporary directory: $OUTPUT" >&2
    exit 1
    ;;
esac

bash "$ROOT/scripts/verify-sample-reports.sh" "$ROOT/examples/reports"

rm -rf "$OUTPUT"
mdbook build "$ROOT/docs" --dest-dir "$OUTPUT"
touch "$OUTPUT/.nojekyll"

mkdir -p "$OUTPUT/reports"
for REPORT in snippet-local docker repository-local repository-docker; do
  cp -R "$ROOT/examples/reports/$REPORT" "$OUTPUT/reports/$REPORT"
done

echo "Pages artifact written to $OUTPUT"
