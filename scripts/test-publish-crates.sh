#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgetest-publish-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

BIN="$TEST_ROOT/bin"
STATE="$TEST_ROOT/state"
mkdir -p "$BIN" "$STATE"

cat >"$BIN/cargo" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  metadata)
    cat <<JSON
{"packages":[
  {"name":"forgetest-core","version":"0.1.0"},
  {"name":"forgetest-agents","version":"0.1.0"},
  {"name":"forgetest-providers","version":"0.1.0"},
  {"name":"forgetest-runner","version":"0.1.0"},
  {"name":"forgetest-report","version":"0.1.0"},
  {"name":"forgetest-cli","version":"0.1.0"}
],"target_directory":"$PACKAGE_TARGET"}
JSON
    ;;
  package)
    package=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--package" ]; then
        package="$2"
        break
      fi
      shift
    done
    test -n "$package"
    mkdir -p "$PACKAGE_TARGET/package"
    printf 'crate:%s:0.1.0\n' "$package" \
      >"$PACKAGE_TARGET/package/$package-0.1.0.crate"
    ;;
  publish)
    package=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--package" ]; then
        package="$2"
        break
      fi
      shift
    done
    test -n "$package"
    printf '%s\n' "$package" >>"$PUBLISH_LOG"
    touch "$REGISTRY_STATE/$package"
    ;;
  *)
    echo "unexpected cargo invocation: $*" >&2
    exit 64
    ;;
esac
SCRIPT

cat >"$BIN/curl" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
output=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      output="$2"
      shift 2
      ;;
    http*)
      url="$1"
      shift
      ;;
    *)
      shift
      ;;
  esac
done
test -n "$output"
test -n "$url"
version="${url##*/}"
without_version="${url%/*}"
package="${without_version##*/}"
if [ "${FAIL_CRATE:-}" = "$package" ]; then
  printf '{"errors":[{"detail":"unavailable"}]}' >"$output"
  printf '503'
elif [ -f "$REGISTRY_STATE/$package" ]; then
  if [ "${MISMATCH_CRATE:-}" = "$package" ]; then
    checksum="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  elif [ ! -f "$PACKAGE_TARGET/package/$package-$version.crate" ]; then
    checksum="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  elif command -v sha256sum >/dev/null 2>&1; then
    checksum="$(sha256sum "$PACKAGE_TARGET/package/$package-$version.crate" | awk '{print $1}')"
  else
    checksum="$(shasum -a 256 "$PACKAGE_TARGET/package/$package-$version.crate" | awk '{print $1}')"
  fi
  printf '{"version":{"num":"%s","checksum":"%s"}}' \
    "$version" "$checksum" >"$output"
  printf '200'
else
  printf '{"errors":[{"detail":"not found"}]}' >"$output"
  printf '404'
fi
SCRIPT

chmod +x "$BIN/cargo" "$BIN/curl"
export PATH="$BIN:$PATH"
export REGISTRY_STATE="$STATE"
export PUBLISH_LOG="$TEST_ROOT/published.log"
export PACKAGE_TARGET="$TEST_ROOT/target"
touch "$STATE/forgetest-core"

bash "$ROOT/scripts/publish-crates.sh" 0.1.0

cat >"$TEST_ROOT/expected.log" <<'EOF'
forgetest-agents
forgetest-providers
forgetest-runner
forgetest-report
forgetest-cli
EOF
diff -u "$TEST_ROOT/expected.log" "$PUBLISH_LOG"

: >"$PUBLISH_LOG"
export MISMATCH_CRATE="forgetest-core"
if bash "$ROOT/scripts/publish-crates.sh" 0.1.0 >/dev/null 2>&1; then
  echo "publisher accepted a mismatched existing crate checksum" >&2
  exit 1
fi
test ! -s "$PUBLISH_LOG"
unset MISMATCH_CRATE

: >"$PUBLISH_LOG"
export FAIL_CRATE="forgetest-core"
if bash "$ROOT/scripts/publish-crates.sh" 0.1.0 >/dev/null 2>&1; then
  echo "publisher ignored a crates.io API failure" >&2
  exit 1
fi
test ! -s "$PUBLISH_LOG"

echo "resumable crates publisher verified"
