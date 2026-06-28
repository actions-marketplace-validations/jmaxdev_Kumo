# Kumo Intelligent Caching System

Kumo incorporates a state-of-the-art caching architecture to optimize both package management and script execution. This document details how Kumo's caching works under the hood and how it drastically accelerates your workflow.

---

## 1. Script Execution Caching (`kumo run`)

Kumo provides **Zero-Config Script Caching** for build processes and custom scripts defined in `package.json` or `kumo.json`. When you run a script (e.g., `kumo build`), Kumo automatically determines if the inputs to that script have changed. If not, it skips execution and instantly replays the previous output.

### How Script Hashing Works

To guarantee correctness, Kumo computes a highly deterministic hash using the **BLAKE3** cryptographic hashing algorithm based on the following inputs:

1. **Script Definition**: The exact command string defined in `package.json` / `kumo.json`.
2. **CLI Arguments**: Any extra flags or arguments passed to `kumo <script>`.
3. **Lockfile State**: The entire resolved dependency tree from `kumo.lock`.
4. **Input Source Files**: Files matched in your project matching standard build input patterns.

#### Default Input Patterns
Kumo applies smart, predefined glob matching for generic, Next.js, and Vite projects:
* `src/**/*`, `public/**/*`
* `pages/**/*`, `app/**/*`, `components/**/*`
* `*.json`, `*.config.*`, `*.ts`, `*.js`, `*.css`, `*.html`
* `.env*`

> [!NOTE]
> Long-running interactive processes (e.g., development servers like `next dev` or `vite dev`) are automatically detected and **never cached** to ensure interactive development flows work perfectly.

#### Customizing Cache Configuration

If the default input patterns do not cover your project structure, or if you want to enable caching for a custom script, you can specify input and output rules inside the `cache` object in your local `kumo.config.json` file. The keys inside the `cache` object correspond to script names in your `package.json` or `kumo.json`.

Each script configuration accepts:
* `inputs`: An array of glob patterns matching files that should trigger a re-run if modified.
* `outputs`: An array of file paths or directories that Kumo should backup and restore upon a cache hit.

Example configuration in `kumo.config.json`:
```json
{
  "cache": {
    "build": {
      "inputs": [
        "src/**/*.ts",
        "src/**/*.json",
        "tsconfig.json"
      ],
      "outputs": [
        "dist"
      ]
    },
    "compile-assets": {
      "inputs": [
        "assets/**/*",
        "tailwind.config.js"
      ],
      "outputs": [
        "public/build"
      ]
    }
  }
}
```

### Cache Storage and Hit Restoring

Script execution logs and outputs are stored in `~/.kumo/cache/scripts/<blake3_hash>`. 

* **Cache Hit**:
  * Kumo prints: `[Kumo] Script cache hit! Replaying previous run...`
  * Automatically replays the saved `stdout` and `stderr` logs to your terminal.
  * Instantly restores generated output directories (like `.next/` or `dist/`) to your project root.
* **Cache Miss**:
  * Kumo executes the script normally.
  * On successful exit (status 0), Kumo captures generated outputs, compresses them, and stores the logs and files under the hash key for future runs.

---

## 2. Package Artifact Caching

During `kumo install` or `kumo add`, Kumo avoids downloading packages repeatedly by maintaining a global package store and local linking cache.

### Structure of the Kumo Store

All downloaded packages are unpacked, validated, and stored in a shared global store:
`~/.kumo/store/<package_name>/<version>/`

### Verification & Checksum Integrity

To mitigate supply chain attacks where a package version is secretly altered or poisoned on the registry:

1. **SHA512 Verification**: Kumo extracts the tarball's integrity field (`sha512-...`) from npm registry metadata.
2. **BLAKE3 Content Integrity**: The unpacked files in the global store are indexed and hashed.
3. **Strict Checksum Match**: Kumo verifies that the downloaded package matches the expected registry checksum. If the registry or a local cache is modified, Kumo detects the mismatch, rejects the installation, and redownloads from a secure connection.

---

## 3. Persistent Resolution Caching

Resolving large, deep dependency trees can be network-heavy and slow. Kumo optimizes this using persistent resolution caches inside `kumo.lock`.

### Fast-Path Restoration

When you run `kumo install`:

1. Kumo computes a hash of the current dependency configuration (`package.json` or `kumo.json`).
2. If this hash matches the `config_hash` field stored inside `kumo.lock`, Kumo skips all registry lookups and tree resolving entirely.
3. It immediately goes into the parallel linking phase using the cached metadata, reducing resolution times to **under 100 milliseconds**.

---

## 4. Cache Maintenance Commands

Kumo provides built-in commands to inspect, audity, and clean up cached assets.

### `kumo stats`
Shows global disk usage statistics, including number of unique files, packages, and total cached size.

### `kumo prune store`
Cleans up the global content-addressable store (`~/.kumo/store`), removing all cached packages and metadata.

### `kumo prune cache`
Cleans up the registry metadata and script caches (`~/.kumo/cache`).

### `kumo prune all`
Cleans up both the global store and the registry cache.

### `kumo doctor`
Scans the global store and runs health checks to ensure no cached package files are corrupted, missing, or altered.
