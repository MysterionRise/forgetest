#!/usr/bin/env bash
# Exercise scripts/build-agent-images.sh against docker and curl stubs: no
# network and no Docker daemon.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgetest-agent-images-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

# A minimal repository copy whose recipe ARGs differ from the real pins, so the
# expected tags prove the script reads them.
REPO="$TEST_ROOT/repo"
mkdir -p "$REPO/scripts" "$REPO/docker/agents"
cp "$ROOT/scripts/build-agent-images.sh" "$REPO/scripts/"
printf '[workspace.package]\nversion = "9.9.9"\n' >"$REPO/Cargo.toml"
printf 'FROM scratch\n' >"$REPO/docker/forgetest-runner-rust.Dockerfile"
printf 'FROM scratch\nARG CODEX_VERSION=7.7.7\n' >"$REPO/docker/agents/codex.Dockerfile"
printf 'FROM scratch\nARG CLAUDE_CODE_VERSION=8.8.8\n' >"$REPO/docker/agents/claude.Dockerfile"

BIN="$TEST_ROOT/bin"
STATE="$TEST_ROOT/state"
mkdir -p "$BIN" "$STATE"

cat >"$BIN/docker" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$DOCKER_LOG"
case "$1 ${2:-}" in
  "buildx version") ;;
  "buildx inspect")
    printf 'Name:          stub\nDriver:        %s\n' "${STUB_BUILDX_DRIVER:-docker}"
    ;;
  "buildx build")
    file=""
    metadata=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --file) file="$2"; shift 2 ;;
        --metadata-file) metadata="$2"; shift 2 ;;
        *) shift ;;
      esac
    done
    case "${file##*/}" in
      forgetest-runner-rust.Dockerfile) digit=1 ;;
      codex.Dockerfile) digit=2 ;;
      claude.Dockerfile) digit=3 ;;
      *) exit 64 ;;
    esac
    digest="sha256:$(printf "%064d" 0 | tr 0 "$digit")"
    printf '{\n  "containerimage.digest": "%s"\n}\n' "$digest" >"$metadata"
    ;;
  "container inspect") exit 1 ;;
  "run --detach")
    if [ "${STUB_DOCKER_RUN_FAIL:-}" = 1 ]; then
      exit 125
    fi
    touch "$STATE/registry-started"
    ;;
  "rm -f" | "pull --platform") ;;
  *)
    echo "unexpected docker invocation: $*" >&2
    exit 64
    ;;
esac
SCRIPT

cat >"$BIN/curl" <<'SCRIPT'
#!/usr/bin/env bash
mode="${STUB_CURL_MODE:-registry}"
if [ "$mode" = refused ] && [ -f "$STATE/registry-started" ]; then
  mode=registry
fi
case "$mode" in
  registry)
    printf 'HTTP/1.1 200 OK\r\nDocker-Distribution-Api-Version: registry/2.0\r\n\r\n'
    ;;
  other)
    printf 'HTTP/1.1 403 Forbidden\r\nServer: AirTunes/870.14.1\r\n\r\n'
    ;;
  refused) exit 7 ;;
  timeout) exit 28 ;;
esac
SCRIPT

chmod +x "$BIN/docker" "$BIN/curl"
export PATH="$BIN:$PATH"
export STATE
export DOCKER_LOG="$TEST_ROOT/docker.log"

OUT="$TEST_ROOT/stdout"
ERR="$TEST_ROOT/stderr"

# Runs the script with the given environment assignments and records its exit.
run_script() {
  : >"$DOCKER_LOG"
  rm -f "$STATE/registry-started"
  STATUS=0
  env -u FORGETEST_IMAGE_REGISTRY "$@" bash "$REPO/scripts/build-agent-images.sh" \
    >"$OUT" 2>"$ERR" || STATUS=$?
}

expect_failure() {
  local expected_status="$1"
  local expected_message="$2"
  if [ "$STATUS" -ne "$expected_status" ]; then
    echo "expected exit $expected_status, got $STATUS" >&2
    cat "$ERR" >&2
    exit 1
  fi
  grep -qF "$expected_message" "$ERR" || {
    echo "missing message: $expected_message" >&2
    cat "$ERR" >&2
    exit 1
  }
  test ! -s "$OUT"
}

run_script FORGETEST_IMAGE_REGISTRY=ghcr.io/example
expect_failure 1 "refusing to push to non-local registry ghcr.io/example"
test ! -s "$DOCKER_LOG"

for registry in localhost localhost: localhost:abc localhost:0 localhost:65536; do
  run_script FORGETEST_IMAGE_REGISTRY="$registry"
  expect_failure 2 "local registry must name a port from 1 to 65535"
  test ! -s "$DOCKER_LOG"
done

for mode in other timeout; do
  run_script STUB_CURL_MODE="$mode"
  expect_failure 1 "something other than a registry answers on 127.0.0.1:5001"
  if grep -q '^run ' "$DOCKER_LOG"; then
    echo "a registry was started over a port conflict ($mode)" >&2
    exit 1
  fi
done

run_script STUB_BUILDX_DRIVER=docker-container
expect_failure 1 "the current buildx builder uses the 'docker-container' driver"
if grep -q '^buildx build' "$DOCKER_LOG"; then
  echo "built with a non-docker buildx driver" >&2
  exit 1
fi

run_script STUB_CURL_MODE=refused STUB_DOCKER_RUN_FAIL=1
expect_failure 1 "could not start registry container forgetest-registry-5001"
grep -qx 'rm -f forgetest-registry-5001' "$DOCKER_LOG"

run_script STUB_CURL_MODE=refused
test "$STATUS" -eq 0 || {
  cat "$ERR" >&2
  exit 1
}
cat >"$TEST_ROOT/expected" <<'EOF'
verifier=localhost:5001/forgetest-runner-rust@sha256:1111111111111111111111111111111111111111111111111111111111111111
codex=localhost:5001/forgetest-codex@sha256:2222222222222222222222222222222222222222222222222222222222222222
claude=localhost:5001/forgetest-claude@sha256:3333333333333333333333333333333333333333333333333333333333333333
EOF
diff -u "$TEST_ROOT/expected" "$OUT"
grep -q -- '--publish 127.0.0.1:5001:5000 registry:2@sha256:' "$DOCKER_LOG"
grep -q -- '--tag localhost:5001/forgetest-runner-rust:9.9.9 ' "$DOCKER_LOG"
grep -q -- '--tag localhost:5001/forgetest-codex:7.7.7 ' "$DOCKER_LOG"
grep -q -- '--tag localhost:5001/forgetest-claude:8.8.8 ' "$DOCKER_LOG"
test "$(grep -c -- '--platform linux/amd64 --provenance=false --sbom=false' "$DOCKER_LOG")" -eq 3
grep -qx 'pull --platform linux/amd64 --quiet localhost:5001/forgetest-codex@sha256:2222222222222222222222222222222222222222222222222222222222222222' "$DOCKER_LOG"

run_script STUB_CURL_MODE=registry
test "$STATUS" -eq 0
diff -u "$TEST_ROOT/expected" "$OUT"
if grep -q '^run ' "$DOCKER_LOG"; then
  echo "started a registry although one was running" >&2
  exit 1
fi

printf 'FROM scratch\nARG CODEX_VERSION=\n' >"$REPO/docker/agents/codex.Dockerfile"
run_script
expect_failure 1 "could not read ARG CODEX_VERSION from docker/agents/codex.Dockerfile"
test ! -s "$DOCKER_LOG"

echo "agent image build script verified"
