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
| `trust_policy` | String | Enforces signature and provenance checks. Set to `"no-downgrade"` to prevent package updates that have a weaker trust level than previously installed releases. Options: `"none"`, `"no-downgrade"`. | `"none"` |
| `trust_policy_exclude` | Array | A list of package names that are excluded from the trust policy check. | `[]` |
| `trust_policy_ignore_after` | Number | The number of minutes after publication to ignore trust verification (allows older releases without provenance). | `10080` (7 days) |
| `protected_packages` | Array | A custom list of highly sensitive packages to protect from typosquatting (e.g., `["react", "next"]`). If empty, only existing project dependencies are protected. | `[]` |

## Mitigating Supply Chain Attacks

Kumo implements several strategies to protect your project from malicious actors in the dependency chain.

### 1. OS-Level Script Sandboxing & Path Isolation
Many supply chain attacks use `postinstall` scripts to execute malicious code. Kumo blocks these scripts by default.

However, you can whitelist specific packages you trust while keeping the global block active:
```json
{
  "allow_postinstall": false,
  "trusted_packages": ["electron", "vite", "esbuild"]
}
```
This allows only the specified packages to run their installation scripts.

To guarantee absolute protection, when scripts are allowed, Kumo executes them inside a **Native OS-Level Sandbox** that isolates the process at runtime:

* **Linux Isolation (Bubblewrap):** If `bwrap` is available, Kumo spawns the script in an isolated namespace (`--unshare-all`). It mounts the base system as read-only (`--ro-bind`) and restricts write access *exclusively* to the package's local directory. It also unshares and disables the network namespace (`--unshare-net`).
* **macOS Isolation (Apple Sandbox):** Kumo leverages the kernel-level `sandbox-exec` utility with a strict custom Scheme profile (`(deny default)`). It denies all network actions and limits file system write permissions solely to the target package directory and system temp folders.
* **Windows Environment & Network Virtualization:** To protect files on Windows without requiring administrative privileges, Kumo redirects the `HOME`, `USERPROFILE`, `APPDATA`, and `LOCALAPPDATA` environment variables to a temporary, empty sandbox folder. Any attempt by a malicious script to resolve the user's home directories to steal `.ssh` or `.aws` keys will only see an empty cage. It also disables outgoing internet requests by poisoning standard connection variables (`HTTP_PROXY`, `HTTPS_PROXY`, `NODE_TLS_REJECT_UNAUTHORIZED`).
* **Environment Variable Whitelisting:** Across all operating systems, Kumo completely clears the host environment variables before spawning the script. It uses a strict whitelist (allowing only essential system variables like `PATH`, `TEMP`, and Node-specific ones) instead of a blacklist. This guarantees that no custom secrets, API keys, or cloud tokens are ever exposed to third-party scripts.

### 2. Typosquatting Protection (Levenshtein Engine)
Attackers often release packages with names very similar to popular ones (e.g. `axois-utils` or `chalk-tempalte`). Kumo provides two layers of defense against this:
1. **Age Threshold (`minimum_release_age`):** Enforces a default 24-hour minimum age for package releases, ensuring freshly published malicious copycats are blocked.
2. **Levenshtein Distance Check:** Compares newly resolved package names against the project's **existing dependencies** and any names you define in your `protected_packages` config array. If a new package has a Levenshtein distance ≤ 2 (or ≤ 3 for longer names) to any of these trusted names, Kumo automatically flags it and halts the installation with a typosquatting warning.

### 3. Vulnerability Scanning
Kumo integrates with the **OSV (Open Source Vulnerabilities)** database. During the resolution phase, it checks every package version. If a vulnerability matches the `min_severity` threshold, the installation is aborted.

### 4. License Compliance
Legal risks are also part of the supply chain. Kumo can ensure that only packages with approved licenses enter your codebase.

### 5. Checksum Integrity
Kumo verifies the integrity of every downloaded tarball using BLAKE3/SHA hashes. If a package is tampered with on the registry, Kumo will detect the mismatch and fail the installation.

### 6. Signature Verification & Trust Policy
Supply chain hijackings often occur when an attacker steals a publisher's credentials and manually publishes a malicious release to override a legitimate one. Manual publishes lack built-in provenance and signatures.

To mitigate this, Kumo tracks three **Trust Levels** based on npm registry signatures and attestations:
1. **High**: Verifiable **SLSA build provenance** via Sigstore and CI/CD OIDC.
2. **Medium**: Standard **Registry Signatures** (PGP or Sigstore signatures).
3. **Low**: **No trust evidence** (manual publish).

You can configure two modes of `trust_policy` in `kumo.config.json`:
* `"no-downgrade"` (default): Compares resolved packages against previously installed versions in `kumo.lock`. If a new version's trust level is **weaker** than the previous version, Kumo halts the installation.
* `"strict"`: Completely blocks installation of **any** package with `Low` trust level (unsigned/no provenance), forcing the user to explicitly whitelist trusted unsigned dependencies in `trust_policy_exclude`.

```bash
Security policy violation: Strict trust policy is active, and package 'some-package' has no digital signatures (TrustLevel: Low)!
```

You can bypass false positives via `trust_policy_exclude` or by using `trust_policy_ignore_after` (which automatically ignores checks on releases published more than X minutes ago).
