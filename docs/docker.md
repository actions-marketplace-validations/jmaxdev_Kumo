# 🐳 Docker

Kumo provides an official Docker image for running Node.js projects with Kumo as their package manager inside containers.

The official Kumo Docker image is minimal and **does not bundle Node.js** by default. Instead, it is based on `debian:stable-slim` and includes Kumo's native runtime manager, allowing you to install and switch to any Node.js version directly in your own image layers.

---

## Quick Start

Create a `Dockerfile` in your project:

```dockerfile
FROM ghcr.io/jmaxdev/kumo

# 1. Install the Node.js version you need (adds it to PATH automatically)
RUN kumo runtime use 22

# 2. Setup your application
WORKDIR /app
COPY package.json kumo.lock ./
RUN kumo install

COPY . .
# run your start script
CMD ["kumo", "start"]
```

Build and run:

```bash
docker build -t my-app .
docker run -p 3000:3000 my-app
```

---

## Available Tags

| Tag | Description |
|---|---|
| `latest` | Latest stable release |
| `next` | Latest pre-release (alpha, beta, rc) |
| `x.y.z` | Specific version (e.g. `0.3.3`) |
| `x.y` | Latest patch for a minor version (e.g. `0.3`) |

```dockerfile
# Pin to a specific version
FROM ghcr.io/jmaxdev/kumo:0.3.3
RUN kumo runtime use 22
```

---

## What's Included

The official Kumo image includes:
- **`debian:stable-slim`** base image (aligned with the GLIBC compile target)
- **`kumo`** binary in PATH
- **`kx`** binary in PATH
- Pre-configured `KUMO_HOME` at `/root/.kumo`
- Exposed Kumo bin directory in PATH (`/root/.kumo/bin`), making Node.js runtimes active for subsequent image layers.
- Exposed port **3000** (default for `Kumo.serve()`)

---

## Production Multi-Stage Example

For optimized production images, you can use a multi-stage build:

```dockerfile
# Stage 1: Install dependencies and Node.js
FROM ghcr.io/jmaxdev/kumo AS deps
RUN kumo runtime use 22
WORKDIR /app
COPY package.json kumo.lock ./
RUN kumo install

# Stage 2: Build application
FROM ghcr.io/jmaxdev/kumo AS builder
RUN kumo runtime use 22
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN kumo ts build

# Stage 3: Production runtime (Clean & minimal)
FROM node:22-slim AS runtime
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=deps /app/node_modules ./node_modules
EXPOSE 3000
CMD ["kumo", "start"]
```

---

## Docker Compose

For local development, you can mount the global store to persist cached packages:

```yaml
services:
  app:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - .:/app
      - kumo-store:/root/.kumo  # Persist global store and installed runtimes
    command: ["kumo", "start"]

volumes:
  kumo-store:
```

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `KUMO_HOME` | `/root/.kumo` | Kumo home directory (store, cache, config) |

---

## Architecture Support

The official image is built for:
- `linux/amd64` (x86_64)
- `linux/arm64` (Apple Silicon, AWS Graviton)

Docker will automatically pull the correct image for your host platform.
