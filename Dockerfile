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

# Runtime image with Node.js
FROM node:22-slim

LABEL org.opencontainers.image.title="Kumo Package Manager" \
      org.opencontainers.image.description="High-performance, security-first package manager for Node.js" \
      org.opencontainers.image.source="https://github.com/jmaxdev/Kumo" \
      org.opencontainers.image.vendor="JMaxDev" \
      org.opencontainers.image.licenses="UPL-1.0"

# Copy Kumo precompiled binaries from build context based on target arch
ARG TARGETARCH
COPY bin/${TARGETARCH}/kumo /usr/local/bin/kumo
COPY bin/${TARGETARCH}/kx /usr/local/bin/kx

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
