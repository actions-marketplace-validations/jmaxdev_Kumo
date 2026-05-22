# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Tarball SHA-1 checksum verification.
- Enforced security policy checks on global installations.
- CI workflows for automated testing and linting (`fmt`, `clippy`).
- Unit tests for tarball verification and common utilities.

### Changed
- Refactored `resolver` to use dedicated `types.rs` and `cache.rs` modules.
- Refactored Windows security sandbox network disabling.
- Improved PATH separator logic (`prepend_to_path`) for cross-platform compatibility.
- Replaced manual lockfile string splitting with `parse_package_id` in `explain.rs`, `kx.rs`, and others.
- Migrated `serde_yaml` to maintained `serde_yml`.
- Updated `reqwest`, `zip`, and `blake3` versions.

### Fixed
- Fixed scoped package parsing bug (e.g. `@scope/name@version`).
- Disabled HTTP/2 prior knowledge to fix connectivity with public registries.
- Fixed `build.ps1` binary references.
- Corrected "Transient" dependency typo in `explain.rs`.
- Fixed error ordering in install pipeline so progress bars don't mask errors.
- Handled `dirs::home_dir()` unwraps gracefully.
