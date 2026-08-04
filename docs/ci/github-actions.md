# Kumo in GitHub Actions

Integrate Kumo into your GitHub Actions workflow for zero-trust, ultra-fast CI/CD builds.

## Quick Start

Add `jmaxdev/kumo@v1` to your workflow:

```yaml
name: CI Pipeline

on:
  push:
    branches: [main]
  pull_request:

jobs:
  build-and-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Kumo & Restore Cache
        uses: jmaxdev/kumo@v1
        with:
          version: 'latest'
          cache: true

      - name: Run Kumo CI Pipeline
        run: kumo ci --format=sarif > kumo-audit.sarif

      - name: Upload Security Report to GitHub
        uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: kumo-audit.sarif

      - name: Run Tests
        run: kumo run test
```

## Production Hardening Features

- **Frozen Lockfile**: `kumo ci` ensures dependencies match `kumo.lock` exactly.
- **Zero-Trust Scripts**: Lifecycle scripts are disabled by default (`--ignore-scripts`) and environment secrets (`GITHUB_TOKEN`, `AWS_*`) are purged before execution.
- **SARIF Security Reporting**: Automatically export vulnerability findings to GitHub Security Code Scanning tab.
