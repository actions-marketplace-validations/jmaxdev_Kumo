# ============================================================
# Kumo Package Manager - Official Docker Image
# ghcr.io/jmaxdev/kumo
# ============================================================
# Usage:
#   FROM ghcr.io/jmaxdev/kumo
#   WORKDIR /app
#   COPY . .
#   RUN kumo install
#   CMD ["kumo", "start"]
# ============================================================

# ── Stage 1: Build Kumo from source ─────────────────────────
FROM rust:1.87-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies: copy manifests first, then build deps
COPY Cargo.toml Cargo.lock ./
COPY crates/cli/Cargo.toml crates/cli/
COPY crates/core/Cargo.toml crates/core/
COPY crates/resolver/Cargo.toml crates/resolver/
COPY crates/security/Cargo.toml crates/security/

# Create dummy source files so `cargo build` caches dependency compilation
RUN mkdir -p crates/cli/src crates/core/src crates/resolver/src crates/security/src \
    && echo "fn main() {}" > crates/cli/src/main.rs \
    && echo "fn main() {}" > crates/cli/src/kx.rs \
    && touch crates/core/src/lib.rs \
    && touch crates/resolver/src/lib.rs \
    && touch crates/security/src/lib.rs

# Copy build script and assets needed at compile time
COPY crates/cli/build.rs crates/cli/build.rs
COPY crates/cli/assets crates/cli/assets

# Pre-build dependencies (cached layer)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY crates/ crates/

# Touch source files to invalidate cache for the final build
RUN touch crates/cli/src/main.rs crates/cli/src/kx.rs \
    crates/core/src/lib.rs crates/resolver/src/lib.rs crates/security/src/lib.rs

# Build release binaries
RUN cargo build --release \
    && strip target/release/kumo target/release/kx

# ── Stage 2: Runtime image with Node.js ─────────────────────
FROM node:22-slim

LABEL org.opencontainers.image.title="Kumo Package Manager" \
      org.opencontainers.image.description="High-performance, security-first package manager for Node.js" \
      org.opencontainers.image.source="https://github.com/jmaxdev/Kumo" \
      org.opencontainers.image.vendor="JMaxDev" \
      org.opencontainers.image.licenses="UPL-1.0"

# Copy Kumo binaries from builder
COPY --from=builder /build/target/release/kumo /usr/local/bin/kumo
COPY --from=builder /build/target/release/kx /usr/local/bin/kx

# Configure Kumo environment
ENV KUMO_HOME=/root/.kumo
RUN mkdir -p $KUMO_HOME/store $KUMO_HOME/cache

# Default working directory for user projects
WORKDIR /app

# Expose default port for Kumo.serve()
EXPOSE 3000

# Verify installation
RUN kumo --version

ENTRYPOINT ["kumo"]
