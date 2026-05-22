<p align="center">
  <img src="crates/cli/assets/icon.png" width="160" alt="Kumo Package Manager Logo">
</p>

<h1 align="center">Kumo Package Manager</h1>

<p align="center">
  <strong>A high-performance, security-first package manager for the Node.js ecosystem, written in Rust.</strong>
</p>

<p align="center">
  <a href="https://github.com/jmaxdev/Kumo"><img src="https://img.shields.io/badge/Language-Rust-orange?style=for-the-badge&logo=rust" alt="Language: Rust"></a>
  <a href="#-security-policies"><img src="https://img.shields.io/badge/Security-First-brightgreen?style=for-the-badge&logo=shield" alt="Security: Proactive"></a>
  <a href="docs/caching.md"><img src="https://img.shields.io/badge/Caching-BLAKE3-blue?style=for-the-badge" alt="Caching: BLAKE3"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/License-UPL_1.0-blueviolet?style=for-the-badge" alt="License: UPL 1.0"></a>
  <a href="https://github.com/jmaxdev/Kumo/actions/workflows/release.yml/badge.svg">
    <img src="https://github.com/jmaxdev/Kumo/actions/workflows/release.yml/badge.svg" alt="Release"></a>
</p>

---

**Kumo** is designed from the ground up to solve the two biggest challenges in modern JavaScript and TypeScript development: **disk efficiency** and **supply chain security**.

By leveraging a global **Content-Addressable Storage (CAS)** store and a proactive, customizable **Security Policy Engine**, Kumo keeps your development environment clean, incredibly fast, and safe from malicious actors.

---

## ⚡ Key Capabilities

* 📦 **Flexible Module Resolution**: Natively supports both modern `node_modules` (fully compatible with Vite, Next.js, and ESM bundlers) and Kumo's disk-efficient `dependencies` folder directory structure. Automatically adds active folders to `.gitignore` on setup.
* 🔒 **Proactive Supply Chain Protection**: Stops attacks *before* packages hit your disk.
  * **Levenshtein Distance Engine**: Detects and halts typosquatting attempts against the top 100 popular npm packages.
  * **Age Verification**: Blocks newly-published packages (under 24 hours old) to bypass zero-day malicious releases.
  * **Command Sanitization & Interception**: Strips sensitive environment credentials (`AWS_ACCESS_KEY_ID`, `GITHUB_TOKEN`, etc.) from child processes and blocks execution of scripts attempting unauthorized filesystem access to locations like `.ssh` or `.env`.
* ⚡ **Zero-Config Script Caching (`kumo run`)**: Utilizes **BLAKE3** to hash lockfile state, source inputs, and configuration parameters to skip redundant builds and instantly replay execution logs.
* 🛡️ **Digital Signatures & Provenance Enforcement**: Implements multi-level `trust_policy` checks (`none`, `"no-downgrade"`, `"strict"`). Prevents malicious downgrades to unsigned or untrusted packages.
* 🚀 **Temp Exec & Scaffolding (`kx`)**: Includes `kx` (Kumo Execute) to run local binaries, launch remote packages temporarily, or scaffold new projects using commands like `kx create vite`.
* 🎛️ **Interactive Signal-Safe Confirmation**: Signal-safe prompt system ensures standard exits (`Ctrl+C`) leave the terminal clean and in a perfectly functional state.

---

## 🚀 Quick Start & Installation

### Windows (PowerShell)
Run the following command in your terminal:
```powershell
powershell -c "irm https://raw.githubusercontent.com/jmaxdev/Kumo/master/install.ps1 | iex"
```

### Linux / macOS (Bash/Zsh)
Run the following command in your terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/jmaxdev/Kumo/master/install.sh | bash
```

### Build from Source (Manual)
To build the binary manually from source:

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jmaxdev/Kumo.git
   cd Kumo
   ```

2. **Build the release**:
   ```bash
   cargo build --release
   ```

The executable will be located at `target/release/cli` (on Windows `target/release/cli.exe`). You can rename it to `kumo` and add it to your PATH.

---

## 📖 Configuration (`kumo.config.json`)

You can customize Kumo's behavior and security policies by creating a `kumo.config.json` file in your project's root. Generate a default configuration file with:
```bash
kumo config init
```

### Key Configuration Options

| Field | Type | Description | Default |
| :--- | :--- | :--- | :--- |
| `useNodeModules` | Boolean | Force Kumo to use `node_modules` for modern ESM bundlers. | `false` |
| `block_deprecated` | Boolean | Blocks any package that is marked as deprecated in the registry. | `true` |
| `min_severity` | String | Minimum vulnerability severity level to block. (`low`, `medium`, `high`, `critical`) | `"high"` |
| `minimum_release_age` | Number | Minimum age of a package version (in minutes) required for installation. | `1440` (24 hrs) |
| `allow_postinstall` | Boolean | Blocks packages that have lifecycle scripts (`preinstall`, `postinstall`). | `false` |
| `trusted_packages` | Array | Packages allowed to run scripts even if `allow_postinstall` is `false`. | `[]` |
| `trust_policy` | String | Enforces signatures and provenance. Options: `"none"`, `"no-downgrade"`, `"strict"`. | `"none"` |

For a complete breakdown of features, see the [Security & Config Documentation](docs/security.md).

---

## ⚙️ CLI Reference

### Manage Dependencies
* **Install dependencies** from `package.json` or `kumo.json`:
  ```bash
  kumo install
  # Alias: kumo i
  ```
* **Add a new package** to the project:
  ```bash
  kumo add express
  # Dev dependency: kumo add typescript --dev
  # Global install: kumo add rimraf --global
  ```
* **Remove a package**:
  ```bash
  kumo remove express
  # Alias: kumo rm express
  ```
* **Upgrade dependencies** to their latest versions:
  ```bash
  kumo upgrade
  # Specific packages: kumo upgrade express
  # Ignore semver ranges: kumo upgrade --latest
  # Preview only: kumo upgrade --dry-run
  ```

### Maintenance & Diagnostics
* **Scan for vulnerabilities** using OSV:
  ```bash
  kumo scan
  ```
* **Wipe cache / dependencies**:
  ```bash
  kumo prune cache --full
  kumo prune deps --full
  ```
* **Verify store integrity**:
  ```bash
  kumo doctor
  ```
* **Explain dependency path**:
  ```bash
  kumo explain lodash
  ```

For more CLI commands, check out the [Full CLI Documentation](docs/kumo.md).

---

## 🚀 KX: Kumo Execute

`kx` works similarly to `npx`, allowing you to run local workspace binaries or download and execute temporary packages without modifying your `package.json`.

```bash
# Run local TypeScript compiler
kx -p typescript tsc --version

# Run a package temporarily
kx cowsay "Hello, Kumo!"

# Scaffold a project instantly
kx create vite my-new-app
```

Check out the [KX Documentation](docs/kx.md) for more details.

---

## 📊 Performance

Kumo's caching engine and BLAKE3 hashing pipeline lead to ultra-fast installation times. Below is a comparison run on a local development machine:

### Installation Benchmark (Cold / Warm cache)
* **Cold install (no local cache)**: Kumo performs **1.5x faster** than npm and **2.4x faster** than pnpm.
* **Warm install (fully cached)**: Kumo links everything in milliseconds, keeping resolution times **under 100ms** and full installs **1.8x faster** than npm.

Detailed benchmark results and configuration comparison can be found in the [Benchmark Documentation](docs/benchmark.md).

---

## 📄 License

Kumo is licensed under the **[UnSetSoft Public License (UPL) 1.0](LICENSE.md)**.

* ✅ You may use **parts** of the code in other projects with proper attribution
* ❌ You may **not** distribute the original or modified versions
* ❌ You may **not** use it for commercial purposes
* ❌ You may **not** modify the code except for contributive purposes towards the original project

---

## 🤝 Contributing

We welcome community contributions! Please read our [Contributing Guide](docs/contributing.md) to understand the setup process, coding standards, and repository workflows.
