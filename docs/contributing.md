# Contributing to Kumo

Thank you for your interest in contributing to Kumo! We welcome contributions that improve the project's security, efficiency, and features.

## ⚖️ License Note

Before contributing, please review the **[UnSetSoft Public License (UPL) 1.0](../LICENSE.md)**. 
- You may only modify the source code for contributive purposes towards the original project.
- Commercial use is not allowed.
- Distribution of modified versions outside of the original project is not permitted.

By submitting a contribution, you agree to license your work under the same UPL 1.0 terms.

## 🏗️ Project Structure

Kumo is organized as a Rust workspace with several crates:

- **`crates/cli`**: The command-line interface. This is where command parsing and user interaction happen.
- **`crates/core`**: Core logic including Content-Addressable Storage (CAS), package extraction, and linking.
- **`crates/resolver`**: Handles dependency resolution, registry communication, and lockfile generation.
- **`crates/security`**: The proactive security engine that validates packages against policies (vulnerabilities, licenses, etc.).

## 🛠️ Development Setup

To get started with development, you'll need:

1. **Rust**: Install the latest stable version via [rustup.rs](https://rustup.rs/).
2. **Node.js**: Required for running some of the internal scripts and for testing package shims.
3. **Git**: To manage your changes.

### Clone and Build

```bash
git clone https://github.com/jmaxdev/Kumo.git
cd Kumo
cargo build
```

## 🤝 How to Contribute

### 1. Find an Issue or Suggest an Improvement
Check the [GitHub Issues](https://github.com/jmaxdev/Kumo/issues) for existing tasks or open a new one to discuss a feature or bug you've found.

### 2. Coding Standards
- **Rust idiomatic code**: Follow standard Rust conventions. Use `cargo fmt` to format your code.
- **Documentation**: If you add new features or modify existing ones, update the relevant documentation in `docs/`.
- **Security First**: Since Kumo is a security-first tool, ensure your changes do not introduce vulnerabilities and respect the existing security policies.

### 3. Testing
Ensure your changes don't break existing functionality. Run the tests using:

```bash
cargo test
```

### 4. Submitting Changes
Since the UPL 1.0 license restricts distribution of modified versions, all contributions should be submitted as Pull Requests directly to the original repository.

1. Create a branch for your feature or bugfix.
2. Commit your changes with descriptive messages.
3. Push your branch and open a Pull Request.

## 🎯 Priority Areas for Help

We are currently looking for help in the following areas:

- **Security Advisories**: Integrating more security advisory databases.
- **Monorepo Support**: Improving the `workspaces` detection and management.
- **Performance**: Optimizing the parallel download and extraction process.
- **Cross-platform**: Testing and fixing issues on macOS and Linux.
- **Error Handling**: Making error messages more user-friendly and actionable.

---

Thank you for helping make Kumo better!
