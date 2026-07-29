#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:?usage: publish-crates.sh VERSION}"
API_BASE="${FORGETEST_CRATES_IO_API_BASE:-https://crates.io/api/v1/crates}"
POLL_SECONDS="${FORGETEST_REGISTRY_POLL_SECONDS:-15}"
MAX_POLLS="${FORGETEST_REGISTRY_MAX_POLLS:-20}"
PACKAGES=(
  forgetest-core
  forgetest-agents
  forgetest-providers
  forgetest-runner
  forgetest-report
  forgetest-cli
)

METADATA="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT/Cargo.toml")"
TARGET_DIRECTORY="$(jq -er '.target_directory' <<<"$METADATA")"
for PACKAGE in "${PACKAGES[@]}"; do
  OBSERVED_VERSION="$(
    jq -er --arg package "$PACKAGE" \
      '[.packages[] | select(.name == $package) | .version] | if length == 1 then .[0] else error("package missing or duplicated") end' \
      <<<"$METADATA"
  )"
  if [ "$OBSERVED_VERSION" != "$VERSION" ]; then
    echo "release version mismatch for $PACKAGE: expected $VERSION, found $OBSERVED_VERSION" >&2
    exit 1
  fi
done

lookup_version() {
  local package="$1"
  local body="$2"
  curl --silent --show-error --location \
    --user-agent 'forgetest-release-publisher/0.1' \
    --output "$body" \
    --write-out '%{http_code}' \
    "$API_BASE/$package/$VERSION"
}

verify_registry_response() {
  local package="$1"
  local body="$2"
  local checksum="$3"
  jq -e --arg version "$VERSION" --arg checksum "$checksum" \
    '.version.num == $version and .version.checksum == $checksum' \
    "$body" >/dev/null || {
    echo "crates.io identity mismatch for $package $VERSION; version or checksum differs" >&2
    return 1
  }
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

package_checksum() {
  local package="$1"
  local archive="$TARGET_DIRECTORY/package/$package-$VERSION.crate"
  cargo package --package "$package" --no-verify --locked >/dev/null
  if [ ! -f "$archive" ]; then
    echo "cargo package did not create $archive" >&2
    return 1
  fi
  sha256_file "$archive"
}

publish_one() {
  local package="$1"
  local body checksum status poll
  body="$(mktemp "${TMPDIR:-/tmp}/forgetest-crate-response.XXXXXX")"
  trap 'rm -f "$body"' RETURN
  checksum="$(package_checksum "$package")"

  status="$(lookup_version "$package" "$body")"
  case "$status" in
    200)
      verify_registry_response "$package" "$body" "$checksum"
      echo "$package $VERSION already exists on crates.io; skipping"
      return
      ;;
    404)
      ;;
    *)
      echo "crates.io lookup failed for $package $VERSION with HTTP $status" >&2
      return 1
      ;;
  esac

  cargo publish --package "$package" --locked

  poll=1
  while [ "$poll" -le "$MAX_POLLS" ]; do
    status="$(lookup_version "$package" "$body")"
    case "$status" in
      200)
        verify_registry_response "$package" "$body" "$checksum"
        echo "$package $VERSION is visible on crates.io"
        return
        ;;
      404)
        if [ "$poll" -lt "$MAX_POLLS" ]; then
          sleep "$POLL_SECONDS"
        fi
        ;;
      *)
        echo "crates.io propagation check failed for $package $VERSION with HTTP $status" >&2
        return 1
        ;;
    esac
    poll=$((poll + 1))
  done

  echo "$package $VERSION was published but did not become visible on crates.io" >&2
  return 1
}

for PACKAGE in "${PACKAGES[@]}"; do
  publish_one "$PACKAGE"
done
