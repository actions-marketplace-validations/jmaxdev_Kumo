# 🐳 Docker

Kumo provides an official Docker image for running Node.js projects with Kumo as their package manager inside containers.

## Quick Start

Create a `Dockerfile` in your project:

```dockerfile
FROM ghcr.io/jmaxdev/kumo
WORKDIR /app
COPY . .
RUN kumo install
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
| `dev` | Development builds from manual workflow triggers |

```dockerfile
# Pin to a specific version
FROM ghcr.io/jmaxdev/kumo:0.3.3

# Use latest pre-release
FROM ghcr.io/jmaxdev/kumo:next
```

---

## What's Included

The image is based on `node:22-slim` and includes:
- **Node.js 22** (LTS)
- **`kumo`** binary in PATH
- **`kx`** binary in PATH
- Pre-configured `KUMO_HOME` at `/root/.kumo`
- Exposed port **3000** (default for `Kumo.serve()`)

---

## Production Multi-Stage Example

For optimized production images, use a multi-stage build to keep only the installed dependencies:

```dockerfile
# Stage 1: Install dependencies
FROM ghcr.io/jmaxdev/kumo AS deps
WORKDIR /app
COPY package.json kumo.lock ./
RUN kumo install

# Stage 2: Build application
FROM ghcr.io/jmaxdev/kumo AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN kumo ts build

# Stage 3: Production runtime
FROM node:22-slim AS runtime
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=deps /app/node_modules ./node_modules
EXPOSE 3000
CMD ["node", "dist/index.js"]
```

---

## Docker Compose

For local development, use `docker-compose`:

```yaml
services:
  app:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - .:/app
      - kumo-store:/root/.kumo  # Persist global store
    command: ["start"]

volumes:
  kumo-store:
```

```bash
# Install dependencies
docker compose run --rm app install

# Start the application
docker compose up
```

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `KUMO_HOME` | `/root/.kumo` | Kumo home directory (store, cache, config) |

---

## Caching Strategies

### Layer Caching

Copy `package.json` and `kumo.lock` before source code to maximize Docker layer caching:

```dockerfile
FROM ghcr.io/jmaxdev/kumo
WORKDIR /app

# Dependencies layer (cached unless lock changes)
COPY package.json kumo.lock ./
RUN kumo install

# Application layer
COPY . .
CMD ["kumo", "start"]
```

### Volume Mount for CI

Mount the Kumo store as a volume to share cached packages across builds:

```bash
docker run -v kumo-store:/root/.kumo ghcr.io/jmaxdev/kumo install
```

---

## Architecture Support

The official image is built for:
- `linux/amd64` (x86_64)
- `linux/arm64` (Apple Silicon, AWS Graviton)

Docker will automatically pull the correct image for your platform.
