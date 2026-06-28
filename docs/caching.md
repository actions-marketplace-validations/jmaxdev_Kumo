# Kumo Intelligent Caching System

Kumo incorporates a state-of-the-art caching architecture to optimize both package management and script execution. This document details how Kumo's caching works under the hood and how it drastically accelerates your workflow.

---

## 1. Script Execution Caching (`kumo run`)

Kumo provides **Zero-Config Script Caching** specifically for the `"build"` script defined in your `package.json` or `kumo.json`. When you run this script (e.g., `kumo run build`), Kumo automatically determines if the inputs or configuration have changed. If not, it skips execution and instantly restores previous output files and log streams. For any other custom script, caching is disabled by default unless explicitly configured.

### How Script Hashing Works

To guarantee correctness, Kumo computes a highly deterministic hash using the **BLAKE3** cryptographic hashing algorithm based on the following inputs:

1. **Script Definition**: The exact command string defined in `package.json` / `kumo.json`.
2. **Lockfile State**: The entire resolved dependency tree from `kumo.lock`.
3. **Input Source Files**: Files matched in your project matching standard build input patterns.

> [!WARNING]
> Runtime arguments appended to your command invocation (e.g. running `kumo run build -- --production` vs `kumo run build -- --development`) are **not** currently included in the BLAKE3 hash computation. Changing CLI flags at runtime will not invalidate the cache.

#### Default Input Patterns
By default, zero-config script caching only monitors files matching the following patterns when running the `"build"` script:
* Source files: `src/**/*.ts`, `src/**/*.tsx`, `src/**/*.js`, `src/**/*.jsx`, `src/**/*.cjs`, `src/**/*.mjs`
* Root source files: `*.ts`, `*.tsx`, `*.js`, `*.jsx`, `*.cjs`, `*.mjs`
* Library files: `lib/**/*.ts`, `lib/**/*.js`, `lib/**/*.cjs`, `lib/**/*.mjs`
* App & Pages directories: `app/**/*.ts`, `app/**/*.tsx`, `app/**/*.js`, `app/**/*.jsx`, `pages/**/*.ts`, `pages/**/*.tsx`, `pages/**/*.js`, `pages/**/*.jsx`
* Components: `components/**/*.ts`, `components/**/*.tsx`, `components/**/*.js`, `components/**/*.jsx`
* Configuration files: `package.json`, `tsconfig.json`, `vite.config.ts`, `vite.config.js`, `next.config.js`, `next.config.mjs`, `next.config.ts`, `tailwind.config.js`, `tailwind.config.ts`, `postcss.config.js`, `postcss.config.mjs`

The default output directories backed up and restored by Kumo upon a cache hit are:
* `dist`, `build`, `.next`

> [!NOTE]
> Long-running interactive processes (e.g., development servers like `next dev` or `vite dev`) are automatically detected and **never cached** to ensure interactive development flows work perfectly.

#### Customizing Cache Configuration

If you want to enable caching for a custom script or adjust input/output patterns, you can specify input and output rules inside the `cache` object in your local `kumo.config.json` file. The keys inside the `cache` object correspond to script names in your `package.json` or `kumo.json`.

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
  * Kumo prints: `⚡ Kumo Cache Hit: build [hash]`
  * Automatically replays the saved `stdout` and `stderr` logs to your terminal.
  * Instantly restores generated output directories (like `.next/` or `dist/`) to your project root.
* **Cache Miss**:
  * Kumo executes the script normally.
  * On successful exit (status 0), Kumo captures generated outputs, copies them to the cache directory, and stores the logs and files under the hash key for future runs.

---

## 2. Package Artifact Caching

During `kumo install` or `kumo add`, Kumo avoids downloading packages repeatedly by maintaining a global package store and local linking cache.

### Structure of the Kumo CAS Store

Kumo implements a global **Content-Addressable Storage (CAS)** system inside `~/.kumo/store/` to optimize disk space and prevent file duplication across different projects and dependency versions:

1. **Content Objects (`~/.kumo/store/objects/`)**: All package files are unpacked, and each file is stored individually under a directory structure based on its **BLAKE3** hash: `~/.kumo/store/objects/[first_2_chars_of_hash]/[remaining_hash]`. If multiple packages or different versions share identical files, they are stored only once on disk.
2. **Metadata Indices (`~/.kumo/store/metadata/`)**: Package structures are represented by JSON index files named after the package name and version (e.g., `~/.kumo/store/metadata/safe-pkg-name__1.0.0.json`). Each index file maps the relative package file path to its corresponding BLAKE3 content hash in the objects store.

### File Linking Cascade

When linking dependencies into your project's dependency directory (`node_modules` or `dependencies`), Kumo reads the package index metadata and links the individual file blobs from `~/.kumo/store/objects/` into the destination folder. It employs a high-performance link cascade:

1. **Reflinks (CoW)**: On supported platforms (e.g. Linux with XFS/Btrfs or macOS with APFS), Kumo attempts to use copy-on-write clone (reflinks) via the `reflink` crate. This creates instant file clones that don't occupy extra disk space but remain safe to modify without affecting the global store.
2. **Hard Links**: If reflinking is not supported or fails, Kumo falls back to creating native filesystem hardlinks.
3. **File Copies**: If hard links fail (such as when your project is on a different drive/partition than your user home directory), Kumo copies the file contents directly.

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
