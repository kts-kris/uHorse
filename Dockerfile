FROM rust:1.93-slim AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p uhorse-hub

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

RUN useradd -m -u 1000 -s /bin/bash -c /app uhorse
RUN mkdir -p /app/data /app/logs /app/config

COPY --from=builder /build/target/release/uhorse-hub /app/uhorse-hub

RUN chown -R uhorse:uhorse /app

USER uhorse
WORKDIR /app

EXPOSE 8765

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8765/api/health || exit 1

VOLUME ["/app/data", "/app/logs"]

ENV RUST_LOG=info \
    UHORSE_CONFIG=/app/config/hub.toml \
    UHORSE_DATA_DIR=/app/data \
    UHORSE_LOG_DIR=/app/logs

CMD ["/app/uhorse-hub", "--config", "/app/config/hub.toml", "--host", "0.0.0.0", "--port", "8765"]
