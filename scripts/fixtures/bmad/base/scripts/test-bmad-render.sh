#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v uv >/dev/null 2>&1; then
  printf 'BMAD render smoke error: uv is required\n' >&2
  exit 1
fi
if ! grep -Fqx '# requires-python = ">=3.11"' "$ROOT/_bmad/scripts/render_skill.py"; then
  printf 'BMAD render smoke error: renderer PEP 723 requirement is missing\n' >&2
  exit 1
fi

if ! output="$(
  UV_PYTHON_DOWNLOADS=never UV_OFFLINE=1 \
    uv run --no-cache "$ROOT/_bmad/scripts/render_skill.py" \
      --project-root "$ROOT" --skill "$ROOT/.agents/skills/bmad-build"
)"; then
  printf 'BMAD render smoke error: bmad-build launcher rendering failed\n' >&2
  exit 1
fi

case "$output" in
  "read and follow $ROOT/_bmad/render/bmad-build/"*"/workflow.md") ;;
  *)
    printf 'BMAD render smoke error: renderer returned an unexpected entrypoint\n' >&2
    exit 1
    ;;
esac
workflow="${output#read and follow }"
if [ ! -f "$workflow" ] || ! grep -Fq '# Build New Preview Workflow' "$workflow"; then
  printf 'BMAD render smoke error: rendered workflow is missing or invalid\n' >&2
  exit 1
fi
printf 'BMAD bmad-build launcher render smoke passed\n'
