# Kumo Package Manager 📦🛡️

Kumo is a high-performance, security-first package manager for the Node.js ecosystem, written in Rust. It focuses on disk efficiency through Content-Addressable Storage (CAS) and proactive security policy enforcement.

## Features
- **Disk Efficiency**: Uses a global CAS and hard-linking.
- **Proactive Security**: Blocks deprecated or vulnerable packages *before* installation.
- **Fast Hashing**: Powered by BLAKE3.
- **Native Performance**: No Node.js runtime required for the package manager itself.

## Installation (Build from Source)
To build a production-ready binary:

```bash
cargo build --release
```

The executable will be located at `target/release/cli` (on Windows `target/release/cli.exe`). You can rename it to `kumo`.

## Usage
### Add a package
```bash
kumo add express
```

### Install from package.json
```bash
kumo install
```

### Security Policies
Kumo uses a policy engine to determine if a package is safe. The default policy:
- Blocks **deprecated** packages.
- Blocks packages with **High/Critical** vulnerabilities.
- Only allows **MIT, Apache-2.0, ISC, BSD** licenses.

## Architecture
- `crates/core`: CAS Store and Linking logic.
- `crates/security`: Policy enforcement engine.
- `crates/resolver`: npm registry client and version resolution.
- `crates/cli`: User interface.
