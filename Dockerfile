# Multi-stage build for Rust tabular-server
FROM rust:1.96 AS builder

WORKDIR /usr/src/tabular-server

# Install build dependencies for native-tls / OpenSSL
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependencies manifest and source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release binary
RUN cargo build --release

# ==========================================
# Runtime Stage
# ==========================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install SSL certificates & curl for healthcheck
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/tabular-server/target/release/tabular-server /app/tabular-server

ENV SERVER_PORT=8420

EXPOSE 8420

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:${SERVER_PORT}/health || exit 1

ENTRYPOINT ["/app/tabular-server"]
