#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-aarch64-unknown-linux-gnu}"

if ! dependency_tree="$(
  cargo tree \
    --locked \
    --package forgetest-cli \
    --target "${TARGET}" \
    --edges normal,build \
    --prefix none 2>&1
)"; then
  printf 'release portability check could not inspect %s\n' "${TARGET}" >&2
  printf '%s\n' "${dependency_tree}" >&2
  exit 1
fi

if grep -Fq "openssl-sys v" <<<"${dependency_tree}"; then
  printf 'release portability check failed: %s depends on native OpenSSL\n' "${TARGET}" >&2
  printf '%s\n' "${dependency_tree}" >&2
  exit 1
fi

printf 'release portability check passed: %s has no native OpenSSL dependency\n' "${TARGET}"
