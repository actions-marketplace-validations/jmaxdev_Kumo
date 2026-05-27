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
</p>

---

**Kumo** is designed from the ground up to solve the two biggest challenges in modern JavaScript and TypeScript development: **disk efficiency** and **supply chain security**.

By leveraging a global **Content-Addressable Storage (CAS)** store powered by **BLAKE3** and a proactive **Security Policy Engine**, Kumo keeps your development environment clean, incredibly fast, and safe from malicious actors.

## ⚡ Key Capabilities

* 📦 **Flexible Module Resolution**: Natively supports both modern `node_modules` (fully compatible with Vite, Next.js, and ESM bundlers) and Kumo's disk-efficient `dependencies` folder directory structure. Automatically adds active folders to `.gitignore`.
* 🔒 **Proactive Supply Chain Protection**: Stops attacks *before* packages hit your disk.
  * **Typosquatting Engine**: Analyzes Levenshtein distances against your existing dependencies and custom protected packages to detect and halt copycat attacks.
  * **Age Verification**: Blocks newly-published packages (e.g., under 24 hours old) to bypass zero-day malicious releases.
  * **OS-Level Script Sandboxing**: Executes allowed lifecycle scripts inside a native OS-level sandbox (`bwrap` on Linux, `sandbox-exec` on macOS, and virtualized constraints on Windows) to prevent unauthorized network and filesystem access.
* ⚡ **Zero-Config Script Caching (`kumo run`)**: Utilizes **BLAKE3** to hash lockfile state, source inputs, and configuration parameters to skip redundant builds and instantly replay execution logs.
* 🛡️ **Digital Signatures & Provenance Enforcement**: Implements multi-level `trust_policy` checks (`strict` or `no-downgrade`). Prevents malicious downgrades to unsigned or untrusted packages.
* 🚀 **TypeScript Native Runtime (`kumo ts`)**: Built-in TypeScript execution environment that automatically downloads and runs compilers (`tsc` or `tsx`) without requiring local dependencies.
* 🛠️ **Temp Exec & Scaffolding (`kx`)**: Includes `kx` (Kumo Execute) to run local binaries, launch remote packages temporarily, or scaffold new projects using commands like `kx create vite`.

## 🚀 Quick Start & Installation

### Windows (PowerShell)
```powershell
powershell -c "irm https://raw.githubusercontent.com/jmaxdev/Kumo/master/install.ps1 | iex"
```

### Linux / macOS (Bash/Zsh)
```bash
curl -fsSL https://raw.githubusercontent.com/jmaxdev/Kumo/master/install.sh | bash
```

### Build from Source (Manual)
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

## 📖 Usage Examples

### 1. Managing Dependencies
```bash
kumo init                  # Initialize kumo.config.json
kumo add express           # Add a production dependency
kumo add typescript --dev  # Add a development dependency
kumo remove express        # Remove a dependency
kumo upgrade               # Upgrade all dependencies within semver ranges
```

### 2. Built-in TypeScript Runtime
You don't need to install `ts-node` or `typescript` locally to run TypeScript files. Kumo handles it transparently:
```bash
kumo ts init                # Generate a tsconfig.json file
kumo ts exec src/index.ts   # Execute a .ts file instantly via tsx
kumo ts build src/index.ts  # Compile TypeScript using tsc
```

### 3. Running Temporary Packages (`kx`)
Run packages without polluting your `package.json` or global environment:
```bash
kx cowsay "Hello, Kumo!"
kx create vite my-app
kx -p typescript tsc --version
```

### 4. Diagnostics & Security
```bash
kumo scan                   # Scan your lockfile against the OSV vulnerability database
kumo doctor                 # Verify the integrity of the BLAKE3 global cache
kumo explain lodash         # Explain why a package is in your dependency tree
```

## 📚 Deep Dive Documentation

For advanced configuration, security policies, and performance benchmarks, refer to our detailed documentation:
* [CLI Command Reference](docs/kumo.md)
* [KX Command Reference](docs/kx.md)
* [Security & Sandboxing Engine](docs/security.md)
* [BLAKE3 Caching & CAS Store](docs/caching.md)
* [Performance Benchmarks](docs/benchmark.md)
* [Contributing Guide](docs/contributing.md)

## 📄 License

Kumo is licensed under the **[UnSetSoft Public License (UPL) 1.0](LICENSE.md)**.
* ✅ You may use **parts** of the code in other projects with proper attribution
* ❌ You may **not** distribute the original or modified versions
* ❌ You may **not** use it for commercial purposes
* ❌ You may **not** modify the code except for contributive purposes towards the original project
