# Claude Code agent image for the forgetest benchmark profile.
#
# This is a recipe only: forgetest never publishes vendor CLI images. Build it
# with scripts/build-agent-images.sh and see docker/agents/README.md.

# rust:1.92.0-bookworm, pinned by index digest. Agents run `cargo test` in /work.
FROM rust:1.92.0-bookworm@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2

ARG TARGETARCH

# Claude Code 2.1.302, native linux-x64 (glibc) binary:
# https://downloads.claude.ai/claude-code-releases/2.1.302/linux-x64/claude
# CLAUDE_CODE_SHA256 is platforms."linux-x64".checksum from the vendor manifest
# https://downloads.claude.ai/claude-code-releases/2.1.302/manifest.json
# It is also the executable_sha256 that `forgetest agents lock` records.
# The pin was copied on 2026-10-18; the build compares the download against
# this pin, not against a manifest fetched at build time.
ARG CLAUDE_CODE_VERSION=2.1.302
ARG CLAUDE_CODE_SHA256=60d7eb7f12fbb28a52533fe8be5884097403562aa02dc73991a021c7067e29a5

RUN set -eu; \
    if [ "${TARGETARCH}" != amd64 ]; then \
        echo "error: agent images are linux/amd64 only, not ${TARGETARCH}" >&2; \
        exit 1; \
    fi; \
    asset="claude-code-releases/${CLAUDE_CODE_VERSION}/linux-x64/claude"; \
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
        --retry 3 --connect-timeout 30 --max-time 900 --output /tmp/claude \
        "https://downloads.claude.ai/${asset}"; \
    if ! echo "${CLAUDE_CODE_SHA256}  /tmp/claude" | sha256sum --check --strict --quiet -; then \
        echo "error: SHA-256 mismatch for ${asset}" >&2; \
        exit 1; \
    fi; \
    install -m 0755 /tmp/claude /usr/local/bin/claude; \
    rm -f /tmp/claude

ENV DISABLE_AUTOUPDATER=1 \
    DISABLE_TELEMETRY=1 \
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1

WORKDIR /work
