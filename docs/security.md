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
| `trust_policy` | String | Enforces signature and provenance checks. Set to `"no-downgrade"` to prevent package updates that have a weaker trust level than previously installed releases. Options: `"none"`, `"no-downgrade"`, `"strict"`. | `"none"` |
| `trust_policy_exclude` | Array | A list of package names that are excluded from the trust policy check. | `[]` |
| `trust_policy_ignore_after` | Number | The number of minutes after publication to ignore trust verification (allows older releases without provenance). | `10080` (7 days) |
| `protected_packages` | Array | A custom list of highly sensitive packages to protect from typosquatting (e.g., `["react", "next"]`). If empty, only existing project dependencies are protected. | `[]` |
| `allowed_domains` | Array | A whitelist of domains from which Kumo is allowed to download package tarballs, preventing lockfile registry poisoning. | `["github.com", "objects.githubusercontent.com", "registry.npmjs.org", "nodejs.org", "localhost"]` |
| `registry` | String | Default registry to use. Supported values are `"npm"`, `"kumo"`, or any custom HTTP/HTTPS URL. | `"npm"` |
| `useNodeModules` | Boolean | If `true`, Kumo will link dependencies into a `node_modules` directory natively for maximum compatibility with standard tools, rather than the default `dependencies` directory. | `false` |
| `cache` | Object | Configures custom inputs and outputs for script caching (`kumo run`). See [caching documentation](caching.md) for schema details. | `{}` |
| `AllowedImportHost` | Array | A whitelist of allowed hostnames for HTTPS module imports in scripts (e.g. `["esm.sh"]`). If empty or omitted, all URL imports are blocked by default. | `[]` |

### Registry Override (`KUMO_REGISTRY`)

You can override the configured package registry URL globally at runtime by setting the `KUMO_REGISTRY` environment variable. When defined, Kumo bypasses any registry URL configured in local or global `kumo.config.json` files and uses this value instead.

```bash
# Override the registry to a local proxy or mirror
export KUMO_REGISTRY="http://localhost:4873"
```

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
* **Windows Environment & Network Virtualization:** To protect files on Windows without requiring administrative privileges, Kumo redirects the `HOME`, `USERPROFILE`, `APPDATA`, and `LOCALAPPDATA` environment variables to a temporary, empty sandbox folder. Any attempt by a malicious script to resolve the user's home directories to steal `.ssh` or `.aws` keys will only see an empty cage. It also disables outgoing internet requests by poisoning standard connection variables (`HTTP_PROXY`, `HTTPS_PROXY`, `NODE_TLS_REJECT_UNAUTHORIZED`). Additionally, sandboxed processes on Windows are associated with a native Job Object (`win32job`) that restricts their working memory limit to **512 MB**, preventing memory exhaustion/DoS attacks.
* **Environment Variable Whitelisting:** Across all operating systems, Kumo completely clears the host environment variables before spawning the script. It uses a strict whitelist (allowing only essential system variables like `PATH`, `TEMP`, and Node-specific ones) instead of a blacklist. This guarantees that no custom secrets, API keys, or cloud tokens are ever exposed to third-party scripts.

### 2. Typosquatting Protection (Levenshtein Engine)
Attackers often release packages with names very similar to popular ones (e.g. `axois-utils` or `chalk-tempalte`). Kumo provides two layers of defense against this:
1. **Age Threshold (`minimum_release_age`):** Enforces a default 24-hour minimum age for package releases, ensuring freshly published malicious copycats are blocked.
2. **Levenshtein Distance Check:** Compares newly resolved package names against the project's **existing dependencies**, popular packages, and any names you define in your `protected_packages` config array.
   To avoid false positives, the typosquatting engine will **skip checks** if:
   * The package name is very short (length ≤ 3).
   * The difference in name length between the packages is > 2.
   * One package name starts with the other as a prefix, followed by a separator (`-`, `_`, or `.`). For example, `express-session` is not flagged against a trusted `express`.
   Otherwise, Kumo flags typosquatting and aborts the installation if:
   * For package names ≤ 10 characters: Levenshtein distance is exactly 1 (e.g. `axois` vs `axios`).
   * For package names > 10 characters: Levenshtein distance is ≤ 2.

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

You can configure three modes of `trust_policy` in `kumo.config.json` (defaults to `"none"`):
* `"none"`: No trust verification check is performed.
* `"no-downgrade"`: Compares resolved packages against previously installed versions in `kumo.lock`. If a new version's trust level is **weaker** than the previous version, Kumo halts the installation.
* `"strict"`: Completely blocks installation of **any** package with `Low` trust level (unsigned/no provenance), forcing the user to explicitly whitelist trusted unsigned dependencies in `trust_policy_exclude`.

```bash
Security policy violation: Strict trust policy is active, and package 'some-package' has no digital signatures (TrustLevel: Low)!
```

You can bypass false positives via `trust_policy_exclude` or by using `trust_policy_ignore_after` (which automatically ignores checks on releases published more than X minutes ago).

### 7. Kumo Shield (File System Protection)

Malware often tries to modify existing dependencies, configuration files, or cache objects silently in the background. **Kumo Shield** provides an immutable layer of defense by utilizing native OS read-only attributes.

To enable Kumo Shield:
```bash
kumo shield on
```

When active, Kumo Shield enforces the following protections:
1. **Global Cache Protection:** Every package extracted into `~/.kumo/store/objects` is marked as **Read-Only**. Because Kumo uses hardlinks to copy files into your project's `dependencies/` folder by default, these files will also be immutable across all your local projects. This completely breaks any script attempting to modify its own or other packages' source code.
2. **Configuration Protection:** `kumo.config.json` (both global and local) and `kumo.lock` are automatically marked as Read-Only immediately after Kumo finishes modifying them.

#### The "Armored Door" (TTY-Gated Unlock)

If you need to manually edit your configuration files while the shield is active, you cannot simply open them in your code editor (they are read-only). You must explicitly unlock them using:

```bash
kumo unlock kumo.config.json
```

**The Anti-Malware Trap:** Automated scripts and malware operate in the background without a real terminal (TTY). The `unlock` command checks for a genuine interactive terminal session and requires human confirmation (`Are you sure you want to unlock...? (y/N)`). If a script attempts to pipe input (e.g. `echo "y" | kumo unlock ...`), Kumo will detect the lack of a real TTY and immediately reject the request.

Once you have finished editing, simply run `kumo lock` or execute any Kumo installation command to automatically re-seal the files.

### 8. Lockfile Validation & Registry Poisoning Protection

To prevent lockfile hijacking or registry poisoning attacks, where an attacker alters `kumo.lock` to point dependency downloads to malicious servers, Kumo executes strict verification of the resolved URLs in the lockfile:

1. **Secure Schemes Only**: Tarball URLs must use secure protocols: `https://`, `git+https://`, or `git+ssh://`. Insecure or raw `http://` protocols are blocked.
2. **Domain Whitelisting**: The hostname of each download URL is matched against the whitelisted domains configured in `allowed_domains`. If the domain is not whitelisted, the installation aborts.
3. **URL Name Association**: The URL must contain the plaintext or URL-encoded version of the package name (e.g., the URL for `express` must contain `/express/` or `express-`). This prevents downloading a malicious package from a whitelisted registry using a hijacked package name entry in the lockfile.
4. **Integrity Hash Enforcement**: Every package entry in the lockfile must possess a valid, non-empty `shasum` integrity signature of at least 40 hexadecimal characters.

### 9. Secure HTTPS Module Loader

When running TypeScript/JavaScript scripts using the `kumo ts exec` execution environment, Kumo registers a secure custom ESM import loader. This loader allows you to directly import remote modules via HTTPS (e.g. `import confetti from "https://esm.sh/canvas-confetti"`) while enforcing strict security rules to prevent common supply chain and network-based exploits:

1. **No Insecure HTTP Imports**: Any attempts to import scripts using the `http://` protocol are immediately blocked to prevent man-in-the-middle (MitM) code injection.
2. **SSRF and Loopback Protection**: To prevent SSRF (Server-Side Request Forgery) attacks or local privilege escalation, the loader restricts imports from loopback addresses or local machines. Any imports referencing the following hostnames/IPs are blocked:
   - `localhost`
   - `127.0.0.1`
   - `[::1]`
   - `0.0.0.0`
3. **Allowed Import Hosts (Whitelist)**: By default, **all HTTPS module imports are blocked** to prevent unauthorized remote code execution. To allow imports from trusted registries or CDNs, you must configure the whitelisted hosts list under the key `AllowedImportHost` (also accepts `allowedImportHosts`) in your configuration files:
   - **`kumo.config.json`**:
     ```json
     {
       "AllowedImportHost": ["esm.sh", "cdn.jsdelivr.net"]
     }
     ```
   - **`kumo.config.js`**:
     ```javascript
     module.exports = {
       AllowedImportHost: ["esm.sh", "cdn.jsdelivr.net"]
     };
     ```
   If a script attempts to import from an unlisted host, it will fail execution with a security restriction warning in English explaining that imports from that host are blocked and instructing how to whitelist the host.
