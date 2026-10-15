#!/usr/bin/env bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HELPER="$SCRIPT_DIR/verify_bmad.py"

for HELPER_NAME in \
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
  if [ ! -f "$SCRIPT_DIR/$HELPER_NAME" ]; then
    printf 'BMAD verification error: missing verifier helper: scripts/%s\n' \
      "$HELPER_NAME" >&2
    exit 1
  fi
done

supports_bmad_verifier() {
  "$1" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)' \
    >/dev/null 2>&1
}

PYTHON_BIN=""
if command -v python3.10 >/dev/null 2>&1 && supports_bmad_verifier python3.10; then
  PYTHON_BIN="$(command -v python3.10)"
elif command -v python3 >/dev/null 2>&1 && supports_bmad_verifier python3; then
  PYTHON_BIN="$(command -v python3)"
fi

if [ -z "$PYTHON_BIN" ]; then
  printf 'BMAD verification error: Python 3.10 or newer is required\n' >&2
  exit 1
fi

exec "$PYTHON_BIN" -B "$HELPER" "$@"
