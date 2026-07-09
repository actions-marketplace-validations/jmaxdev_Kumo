# ============================================================
# Kumo Package Manager - Official Docker Image
# ghcr.io/jmaxdev/kumo
# ============================================================
# Usage:
#   FROM ghcr.io/jmaxdev/kumo
#   RUN kumo runtime use 22
#   WORKDIR /app
#   COPY . .
#   RUN kumo install
#   CMD ["node", "index.js"]
# ============================================================

# Runtime image without Node.js (Based on Debian Stable Slim)
FROM debian:stable-slim

LABEL org.opencontainers.image.title="Kumo Package Manager" \
      org.opencontainers.image.description="High-performance, security-first package manager for Node.js" \
      org.opencontainers.image.source="https://github.com/jmaxdev/Kumo" \
      org.opencontainers.image.vendor="JMaxDev" \
      org.opencontainers.image.licenses="UPL-1.0"

# Install ca-certificates needed for HTTPS downloads (runtimes, package registry)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy Kumo precompiled binaries from build context based on target arch
ARG TARGETARCH
COPY bin/${TARGETARCH}/kumo /usr/local/bin/kumo
COPY bin/${TARGETARCH}/kx /usr/local/bin/kx

# Configure Kumo environment
ENV KUMO_HOME=/root/.kumo
ENV PATH="/root/.kumo/bin:$PATH"

RUN mkdir -p $KUMO_HOME/store $KUMO_HOME/cache

# Default working directory for user projects
WORKDIR /app

# Expose default port for Kumo.serve()
EXPOSE 3000

# Verify installation of Kumo itself
RUN kumo --version

ENTRYPOINT ["kumo"]
