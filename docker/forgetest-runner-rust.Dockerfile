FROM rust:1.92.0-bookworm

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin forgetest

WORKDIR /opt/forgetest-cache
COPY docker/runner-cache/Cargo.toml docker/runner-cache/Cargo.lock ./
COPY docker/runner-cache/src/ ./src/
RUN cargo fetch --locked \
    && rm -rf /opt/forgetest-cache

ENV CARGO_HOME=/usr/local/cargo \
    CARGO_NET_OFFLINE=true \
    HOME=/home/forgetest

USER 10001:10001
WORKDIR /work
