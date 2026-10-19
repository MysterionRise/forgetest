# Codex CLI agent image for the forgetest benchmark profile.
#
# This is a recipe only: forgetest never publishes vendor CLI images. Build it
# with scripts/build-agent-images.sh and see docker/agents/README.md.

# rust:1.92.0-bookworm, pinned by index digest. Agents run `cargo test` in /work.
FROM rust:1.92.0-bookworm@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2

ARG TARGETARCH

# Codex CLI 0.163.0: https://github.com/openai/codex/releases/tag/rust-v0.163.0
# CODEX_PACKAGE_SHA256 is the vendor-published checksum of
# codex-package-x86_64-unknown-linux-musl.tar.gz from
# https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-package_SHA256SUMS
# CODEX_SHA256 is the native bin/codex inside that verified package. It equals
# the vendor's standalone codex-x86_64-unknown-linux-musl binary and is the
# executable_sha256 that `forgetest agents lock` records.
# Both pins were copied on 2026-10-18; the build compares the downloads against
# these pins, not against a checksum file fetched at build time.
ARG CODEX_VERSION=0.163.0
ARG CODEX_PACKAGE_SHA256=582646c6bf0561eb0de540f7fc85b9121b0091a826bc6e96830b7f1722122182
ARG CODEX_SHA256=399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215

# The whole package is installed as shipped, so bin/codex finds its packaged
# helpers: bin/codex-code-mode-host, codex-path/rg and codex-resources/bwrap.
# Files are owned by root rather than the archive's build-runner UID.
RUN set -eu; \
    if [ "${TARGETARCH}" != amd64 ]; then \
        echo "error: agent images are linux/amd64 only, not ${TARGETARCH}" >&2; \
        exit 1; \
    fi; \
    asset="codex-package-x86_64-unknown-linux-musl.tar.gz"; \
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
        --retry 3 --connect-timeout 30 --max-time 900 --output "/tmp/${asset}" \
        "https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/${asset}"; \
    if ! echo "${CODEX_PACKAGE_SHA256}  /tmp/${asset}" | sha256sum --check --strict --quiet -; then \
        echo "error: SHA-256 mismatch for ${asset}" >&2; \
        exit 1; \
    fi; \
    package="/opt/codex-${CODEX_VERSION}"; \
    mkdir "${package}"; \
    tar -xzf "/tmp/${asset}" --no-same-owner -C "${package}"; \
    rm -f "/tmp/${asset}"; \
    if ! echo "${CODEX_SHA256}  ${package}/bin/codex" | sha256sum --check --strict --quiet -; then \
        echo "error: SHA-256 mismatch for bin/codex in ${asset}" >&2; \
        exit 1; \
    fi; \
    ln -s "${package}/bin/codex" /usr/local/bin/codex

# System-level Codex settings for every UID: no update check and no analytics.
# The benchmark profile's --ignore-user-config skips only
# $CODEX_HOME/config.toml, so this layer still applies. It holds no provider,
# endpoint, model or auth settings.
RUN mkdir -p /etc/codex \
    && printf '%s\n' \
        'check_for_update_on_startup = false' \
        '' \
        '[analytics]' \
        'enabled = false' \
        >/etc/codex/config.toml

WORKDIR /work
