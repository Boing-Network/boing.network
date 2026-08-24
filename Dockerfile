# Multi-stage image for Fly.io (and any OCI host): release `boing-node` only.
# Build from the repository root:
#   docker build -t boing-node:testnet .
# Desktop-hub is stripped from the workspace so the image does not need Tauri.

FROM rust:1-bookworm AS builder
WORKDIR /src

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev clang cmake \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN sed -i '/desktop-hub/d' Cargo.toml

RUN cargo build --release -p boing-node \
    && (strip target/release/boing-node || true)

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/boing-node /usr/local/bin/boing-node
COPY deploy/fly/entrypoint.sh /usr/local/bin/entrypoint.sh
# Run as root so Fly volume mounts on /data are writable without an extra chown step.
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/boing-node \
    && mkdir -p /data

WORKDIR /data
EXPOSE 8545 4001
VOLUME ["/data"]
HEALTHCHECK --interval=15s --timeout=5s --start-period=45s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8545/live >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
