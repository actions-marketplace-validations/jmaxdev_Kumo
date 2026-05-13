# Kumo Security and Configuration

Kumo is built with a "Security First" philosophy. It includes a proactive Security Engine that validates packages against configurable policies before they are even downloaded.

## Configuration File (`kumo.config.json`)

You can customize Kumo's behavior and security policies by creating a `kumo.config.json` file in your project's root.

To generate a default configuration file, run:
```bash
kumo config init
```

### Configuration Options

| Field | Type | Description | Default |
| :--- | :--- | :--- | :--- |
| `block_deprecated` | Boolean | If `true`, Kumo will block any package that is marked as deprecated in the registry. | `true` |
| `min_severity` | String | The minimum vulnerability severity level to block. Options: `low`, `medium`, `high`, `critical`. | `"high"` |
| `blocked_packages` | Array | A list of specific package names that are always blocked. | `[]` |
| `allowed_licenses` | Array | A list of SPX license identifiers that are allowed. If empty, all licenses are allowed. | `["MIT", "Apache-2.0", "ISC", "BSD-3-Clause"]` |
| `minimum_release_age` | Number | The minimum age of a package version (in minutes) required for installation. Helps mitigate typosquatting. | `1440` (24 hours) |
| `allow_postinstall` | Boolean | If `false`, Kumo will block packages that have lifecycle scripts (`preinstall`, `install`, `postinstall`). | `false` |
| `trusted_packages` | Array | A list of packages that are allowed to run scripts even if `allow_postinstall` is `false`. | `[]` |

## Mitigating Supply Chain Attacks

Kumo implements several strategies to protect your project from malicious actors in the dependency chain.

### 1. Script Blocking & Trusted Packages
Many supply chain attacks use `postinstall` scripts to execute malicious code. Kumo blocks these scripts by default. 

However, you can whitelist specific packages you trust while keeping the global block active:
```json
{
  "allow_postinstall": false,
  "trusted_packages": ["electron", "vite", "esbuild"]
}
```
This allows only the specified packages to run their installation scripts.

### 2. Typosquatting Protection
Attackers often release packages with names very similar to popular ones. These are usually detected and removed quickly. By enforcing a `minimum_release_age` (default 24h), Kumo ensures you don't install a "poisoned" package before it can be reported.

### 3. Vulnerability Scanning
Kumo integrates with the **OSV (Open Source Vulnerabilities)** database. Durante la fase de resolución, comprueba cada versión de los paquetes. Si una vulnerabilidad coincide con el umbral de `min_severity`, la instalación se aborta.

### 4. License Compliance
Legal risks are also part of the supply chain. Kumo can ensure that only packages with approved licenses enter your codebase.

### 5. Checksum Integrity
Kumo verifies the integrity of every downloaded tarball using BLAKE3/SHA hashes. If a package is tampered with on the registry, Kumo will detect the mismatch and fail the installation.
