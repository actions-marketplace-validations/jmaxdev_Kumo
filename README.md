<p align="center">
  <img src="crates/cli/assets/icon.png" width="120" alt="Kumo Package Manager Logo">
</p>

<h1 align="center">Kumo</h1>

<p align="center">
  <strong>High-performance, security-first package manager for the Node.js ecosystem, written in Rust.</strong>
</p>

<p align="center">
  <a href="https://github.com/jmaxdev/Kumo"><img src="docs/assets/badge-rust.svg" alt="Language: Rust"></a>
  &nbsp;&nbsp;
  <a href="docs/caching.md"><img src="docs/assets/badge-caching.svg" alt="Caching: BLAKE3"></a>
  &nbsp;&nbsp;
  <a href="LICENSE.md"><img src="docs/assets/badge-license.svg" alt="License: UPL 1.0"></a>
</p>

---

## ⚡ Overview

Kumo addresses two primary goals in modern JavaScript and TypeScript workflows: **disk efficiency** and **supply chain security**.

It implements a global **Content-Addressable Storage (CAS)** system powered by **BLAKE3** caching and exposes a proactive **Security Policy Engine** that validates package integrity, licensing, and releases before writing data to disk.

---

## 🚀 Installation

### Windows (PowerShell)
```powershell
Invoke-WebRequest https://kumo.unsetsoft.com/install.ps1 -UseBasicParsing | Invoke-Expression
```

### macOS & Linux (Bash/Zsh)
```bash
curl -fsSL https://kumo.unsetsoft.com/install.sh | bash
```

### Build from Source
```bash
git clone https://github.com/jmaxdev/Kumo.git
cd Kumo
cargo build --release
```
The executable is generated at `target/release/kumo` (on Windows `target/release/kumo.exe`).

---

## 🐳 Docker

Use the official Docker image to run Node.js projects with Kumo:

```dockerfile
FROM ghcr.io/jmaxdev/kumo
RUN kumo runtime use 22
WORKDIR /app
COPY . .
RUN kumo install
CMD ["start"]
```

```bash
docker build -t my-app .
docker run -p 3000:3000 my-app
```

See the full **[Docker Documentation](docs/docker.md)** for production patterns, available tags, and caching strategies.

---

## 📖 Documentation Reference

Detailed documentation is organized in the following sections:

* 📚 **[CLI Command Reference](docs/kumo.md)** - Details on dependency operations, configuration management, and TypeScript tools.
* 📦 **[KX Execute Reference](docs/kx.md)** - Complete documentation on temporary package execution and scaffolding.
* 🛡️ **[Security Engine & Sandboxing](docs/security.md)** - Explanations on OS-level isolation, typosquatting checks, and trust levels.
* ⚡ **[BLAKE3 Caching & CAS Store](docs/caching.md)** - Overview of Kumo's zero-config script caching and artifact store.
* 📈 **[Performance Benchmarks](docs/benchmark.md)** - Statistical comparisons against npm, pnpm, and bun.
* 🐳 **[Docker Support](docs/docker.md)** - Official Docker image usage, production patterns, and CI caching.
* 🤝 **[Contributing Guide](docs/contributing.md)** - Rules and processes for contributors under the UPL 1.0.

---

## 📄 License

Kumo is licensed under the **[UnSetSoft Public License (UPL) 1.0](LICENSE.md)**.
- Attribution required for code reuse.
- Commercial use is not allowed.
- Modification is permitted only for contributing to the original project.
- Distribution of modified versions is not permitted.
