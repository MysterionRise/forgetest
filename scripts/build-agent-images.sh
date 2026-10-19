#!/usr/bin/env bash
# Build the verifier, Codex and Claude Code images for linux/amd64, push them
# to a registry the operator controls, and print the immutable references that
# `forgetest agents lock` takes. Build output goes to stderr; stdout carries
# only NAME=REF@sha256:DIGEST lines.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="linux/amd64"
# registry 2.8.3, pinned by index digest.
REGISTRY_IMAGE="registry:2@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373"

usage() {
  cat <<'EOF'
usage: scripts/build-agent-images.sh [--allow-remote]

Builds the verifier, Codex and Claude Code images for linux/amd64, pushes them
to ${FORGETEST_IMAGE_REGISTRY:-localhost:5001}, pulls them back by digest so
the local Docker daemon has them, and prints NAME=REF@sha256:DIGEST lines.
A local registry that is not running is started from a pinned registry:2 and
bound to 127.0.0.1.

  --allow-remote  allow a FORGETEST_IMAGE_REGISTRY that is not on localhost.
                  Keep such a registry private: forgetest never publishes
                  vendor CLI images.
EOF
}

ALLOW_REMOTE=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --allow-remote) ALLOW_REMOTE=true ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

valid_port() {
  case "$1" in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "${#1}" -le 5 ] && [ "$((10#$1))" -ge 1 ] && [ "$((10#$1))" -le 65535 ]
}

REGISTRY="${FORGETEST_IMAGE_REGISTRY:-localhost:5001}"
REGISTRY="${REGISTRY%/}"
REGISTRY_HOST="${REGISTRY%%/*}"
case "$REGISTRY_HOST" in
  localhost | localhost:* | 127.0.0.1 | 127.0.0.1:*) LOCAL_REGISTRY=true ;;
  *) LOCAL_REGISTRY=false ;;
esac

if [ "$LOCAL_REGISTRY" = true ]; then
  REGISTRY_PORT=""
  case "$REGISTRY_HOST" in
    *:*) REGISTRY_PORT="${REGISTRY_HOST##*:}" ;;
  esac
  if ! valid_port "$REGISTRY_PORT"; then
    echo "local registry must name a port from 1 to 65535, for example localhost:5001: $REGISTRY" >&2
    exit 2
  fi
elif [ "$ALLOW_REMOTE" = false ]; then
  echo "refusing to push to non-local registry $REGISTRY; pass --allow-remote to permit it" >&2
  exit 1
fi

# Prints the value of `ARG NAME=VALUE` in a recipe, failing if it is empty.
recipe_arg() {
  local value
  value="$(sed -n "s/^ARG $2=//p" "$ROOT/$1" | head -n 1)"
  if [ -z "$value" ]; then
    echo "could not read ARG $2 from $1" >&2
    exit 1
  fi
  printf '%s\n' "$value"
}

WORKSPACE_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
if [ -z "$WORKSPACE_VERSION" ]; then
  echo "could not read the workspace version from Cargo.toml" >&2
  exit 1
fi
CODEX_VERSION="$(recipe_arg docker/agents/codex.Dockerfile CODEX_VERSION)"
CLAUDE_CODE_VERSION="$(recipe_arg docker/agents/claude.Dockerfile CLAUDE_CODE_VERSION)"

for TOOL in docker curl; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "$TOOL is required" >&2
    exit 1
  }
done
docker buildx version >/dev/null 2>&1 || {
  echo "docker buildx is required" >&2
  exit 1
}

# Inside a docker-container builder, localhost is the builder container, so a
# push to a local registry would miss the host's registry.
if [ "$LOCAL_REGISTRY" = true ]; then
  BUILDER_DRIVER="$(docker buildx inspect 2>/dev/null | sed -n 's/^Driver: *//p' | head -n 1)"
  if [ "$BUILDER_DRIVER" != docker ]; then
    echo "the current buildx builder uses the '${BUILDER_DRIVER:-unknown}' driver;" >&2
    echo "a local registry needs a docker-driver builder, for example: docker buildx use default" >&2
    exit 1
  fi
fi

# Prints "registry", "other" or "none" for what answers on the local port.
# Only a refused connection (curl exit 7) is "none"; any other listener or
# failure is "other", so a port conflict is reported instead of masked. The
# probe uses 127.0.0.1, where the started registry is bound, because
# "localhost" may resolve to an unrelated IPv6 listener.
probe_registry() {
  local headers
  local status=0
  headers="$(curl --silent --max-time 5 --output /dev/null --dump-header - \
    "http://127.0.0.1:$REGISTRY_PORT/v2/" 2>/dev/null)" || status=$?
  if [ "$status" -eq 7 ]; then
    echo none
  elif [ "$status" -eq 0 ] &&
    grep -qi '^docker-distribution-api-version: *registry/2\.0' <<<"$headers"; then
    echo registry
  else
    echo other
  fi
}

ensure_local_registry() {
  local container="forgetest-registry-$REGISTRY_PORT"
  case "$(probe_registry)" in
    registry) return 0 ;;
    other)
      echo "something other than a registry answers on 127.0.0.1:$REGISTRY_PORT;" >&2
      echo "set FORGETEST_IMAGE_REGISTRY=localhost:PORT to a free port or free this one." >&2
      echo "(On macOS, AirPlay Receiver holds port 5000.)" >&2
      exit 1
      ;;
  esac
  if docker container inspect "$container" >/dev/null 2>&1; then
    echo "starting existing registry container $container" >&2
    docker start "$container" >/dev/null
  else
    echo "starting $REGISTRY_IMAGE as $container on 127.0.0.1:$REGISTRY_PORT" >&2
    if ! docker run --detach --name "$container" \
      --publish "127.0.0.1:$REGISTRY_PORT:5000" "$REGISTRY_IMAGE" >/dev/null; then
      docker rm -f "$container" >/dev/null 2>&1 || true
      echo "could not start registry container $container" >&2
      exit 1
    fi
  fi
  for _ in $(seq 1 30); do
    if [ "$(probe_registry)" = registry ]; then
      return 0
    fi
    sleep 1
  done
  echo "registry on 127.0.0.1:$REGISTRY_PORT did not become ready" >&2
  exit 1
}

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

REFERENCES=()

# Builds one image, pushes it, pulls it back by digest, and records its line.
build_image() {
  local name="$1"
  local repository="$2"
  local tag="$3"
  local dockerfile="$4"
  local metadata="$WORK/$name.json"
  local digest
  # No provenance or SBOM attestations: the push is then a single image
  # manifest, whose digest the daemon pulls and the lock records.
  docker buildx build \
    --platform "$PLATFORM" \
    --provenance=false \
    --sbom=false \
    --file "$ROOT/$dockerfile" \
    --tag "$REGISTRY/$repository:$tag" \
    --metadata-file "$metadata" \
    --push \
    "$ROOT" >&2
  digest="$(grep -o '"containerimage\.digest": *"sha256:[0-9a-f]\{64\}"' "$metadata" |
    grep -o 'sha256:[0-9a-f]\{64\}')" || {
    echo "buildx did not report a digest for $name" >&2
    exit 1
  }
  # forgetest runs docker with a cleared environment and no registry login, so
  # every locked image must already be present in the local daemon.
  docker pull --platform "$PLATFORM" --quiet "$REGISTRY/$repository@$digest" >&2
  REFERENCES+=("$name=$REGISTRY/$repository@$digest")
}

if [ "$LOCAL_REGISTRY" = true ]; then
  ensure_local_registry
else
  echo "pushing to non-local registry $REGISTRY; keep it private" >&2
fi

build_image verifier forgetest-runner-rust "$WORKSPACE_VERSION" docker/forgetest-runner-rust.Dockerfile
build_image codex forgetest-codex "$CODEX_VERSION" docker/agents/codex.Dockerfile
build_image claude forgetest-claude "$CLAUDE_CODE_VERSION" docker/agents/claude.Dockerfile

printf '%s\n' "${REFERENCES[@]}"
