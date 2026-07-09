# Kumo CLI Commands

Kumo is a security-first, space-efficient package manager. This document covers the main `kumo` command and its subcommands.

## Usage

```bash
kumo [COMMAND] [OPTIONS]
```

Kumo can execute scripts defined in your `package.json` or `kumo.json`, or run binaries located in your project's `.bin` directory. This works similarly to `npm run` but with automatic `.bin` inyected into the `PATH`.

```bash
kumo <script-name> [args...]
kumo <binary-name> [args...]
```
For example, if you have a `start` script in your `package.json`, you can run it with `kumo start`.

## Dependency Management

Kumo uses a space-efficient approach to dependency management. By default, it stores packages in a global store located at `~/.kumo/store` and links them into your project.

### Dependency Directory Detection
Kumo automatically detects where to link your dependencies:
1. **Configuration Setting**: If you add `"useNodeModules": true` to your `kumo.config.json`, Kumo will natively use `node_modules`. This is highly recommended for full compatibility with modern ESM tools (like Vite, Next.js, etc).
2. **Existing Directory**: If a `node_modules` directory already exists, Kumo will automatically use it.
3. **Default Behavior**: If neither condition is met, Kumo defaults to creating a `dependencies` directory.

> [!TIP]
> Whichever directory Kumo creates for the first time (`node_modules` or `dependencies`), it will automatically attempt to add it to your `.gitignore` file.

## Commands

### `init`
Initializes a new `package.json` file in the current directory. It interactively prompts you for project metadata such as package name, version, description, entry point, author, and license.

- `-y, --yes`: Initializes the project with default values immediately, skipping all interactive prompts.

```bash
kumo init
kumo init --yes
kumo init -y
```

### `install` (alias: `i`)
Installs dependencies from `kumo.json` or `package.json`. It resolves the full dependency tree, generates a `kumo.lock` file, and links packages into the project's dependency directory (detected automatically as `node_modules` or `dependencies`).

- `--log`: Shows detailed progress for each package (resolving, caching, downloading, linking).

```bash
kumo install [--log]
kumo i
```

### `add <name>` (alias: `a`)
Adds a new package to the project.

- `<name>`: The name of the package to add.
- `-d, --dev`: Adds the package as a development dependency.
- `-g, --global`: Installs the package globally.
- `--log`: Shows detailed progress for the package and its dependencies.

```bash
kumo add <package-name> [--dev] [--global] [--log]
kumo a <package-name>
```

### `remove <name>` (alias: `rm`, `un`, `uninstall`)
Removes a package from the project. It updates `kumo.json` or `package.json`, deletes the local package folder, and updates the `kumo.lock` file.

- `<name>`: The name of the package to remove.

```bash
kumo remove <package-name>
kumo rm <package-name>
```

### `scan`
Scans project dependencies for known vulnerabilities using the Kumo Security Engine. Requires a `kumo.lock` file to be present.

```bash
kumo scan
```

### `stats` (alias: `st`)
Shows statistics about the Kumo global store, including the store location, the total number of cached objects (unique files), and disk usage.

```bash
kumo stats
```

### `prune`
Maintenance command to clean up local and global data.

#### `prune store`
Cleans the global content-addressable store (`~/.kumo/store`), removing all cached packages and metadata.

#### `prune cache`
Cleans the registry metadata and scripts cache (`~/.kumo/cache/metadata` and `~/.kumo/cache/scripts`).

#### `prune deps [--full] [--remove-all] [path]`
Deletes the local dependencies directory (e.g. `node_modules` or `dependencies`).
- `--full`: Also delete the `kumo.lock` file in the directory.
- `--remove-all`: Recursively find and remove all dependency directories under the target path.
- `[path]`: The path to start searching or pruning from. Defaults to the current directory (`.`).

#### `prune all`
Cleans both the global store and the registry cache.

```bash
kumo prune store
kumo prune cache
kumo prune deps [--full] [--remove-all] [path]
kumo prune all
```

### `doctor` (alias: `dr`)
Runs a health check on the store to detect and report corrupted files.

```bash
kumo doctor
```

### `explain <name>` (alias: `ex`)
Explains whether a package is registered as a direct dependency or a transitive dependency in the lockfile, and lists its own dependencies.

```bash
kumo explain <package-name>
```

### `link <path>`
Symlinks a local package into the project for development. It reads the package name from `package.json` or `kumo.json` at the given path and creates a symlink in the project's dependency directory.

```bash
kumo link ../my-local-pkg
```

### `unlink <name>`
Removes a symlink created by `kumo link`. It expects the name of the package as it appears in the dependencies directory. Note that this command only removes the symlink; if you need the original package back from the registry, you should run `kumo install` afterwards.

```bash
kumo unlink my-local-pkg
```

### `workspaces`
Detects and lists local packages in a monorepo structure.

```bash
kumo workspaces
```

### `patch <name>`
Extracts a package to `.kumo/patch/<name>` for manual patching.

> [!NOTE]
> Kumo does not currently implement automatic application or tracking of these patches. The command is strictly for extracting package contents into a separate workspace directory to facilitate manual modifications.

```bash
kumo patch <package-name>
```

### `timeline`
Shows a security timeline for the project.

```bash
kumo timeline
```

### `graph`
Generates a Graphviz DOT file (`dependency-graph.dot`) of the project's dependency tree. You can visualize it using tools like `dot`.

```bash
kumo graph
# To visualize: dot -Tsvg dependency-graph.dot -o graph.svg
```

### `sandbox <script>`
Executes a script within the Kumo Sandbox for secure execution.

```bash
kumo sandbox <script-path>
```

### `runtime` (alias: `rt`)
Manages Node.js runtime versions. By default, runtimes are installed globally in `~/.kumo/runtimes/node/`.

#### `runtime use <version>`
Installs and/or switches to the specified Node.js version.

- `<version>`: A version specifier — `latest`, `lts`, a codename (e.g. `iron`), a major version (e.g. `22`), or an exact version (e.g. `22.11.0`).
- `-l, --local`: Switch to or install the runtime locally in the project's `.kumo/runtimes/node/` directory instead of globally.

```bash
# Install and use the latest Node.js version globally
kumo runtime use latest

# Install and use the latest LTS globally
kumo runtime use lts

# Switch to/install a major version globally
kumo runtime use 22

# Switch to/install a specific version locally in the project
kumo runtime use 22.11.0 -l
```

#### `runtime list`
Lists all installed Node.js versions (both global and local) and highlights the currently active ones.

```bash
kumo runtime list
```

#### `runtime remove <version>`
Removes an installed Node.js version. If it was the active version, the active selection and shims are cleared.

- `-l, --local`: Remove a locally installed version instead of a global one.

```bash
kumo runtime remove v20.18.0
kumo runtime remove v22.11.0 -l
```

> [!WARNING]
> **Security Guardrails:**
> - **End of Life (EOL):** Installing or using Node.js versions that have reached EOL (v20 and below) will trigger an interactive confirmation prompt.
> - **Outstanding Security Fixes:** If you attempt to use or install a version that has known vulnerabilities (i.e. a newer version in the same major branch has a security release containing vulnerability fixes), Kumo will display a warning and ask for confirmation.
> - **Non-Interactive Environments:** In non-interactive contexts (CI/CD, scripts, etc.), Kumo will automatically refuse the installation/activation of EOL or vulnerable versions to protect your environment.
> - **Security Badges:** When running `kumo runtime list`, versions that are officially marked as security releases will have a `[Security]` badge next to them. Additionally, when you install or activate a security release, Kumo will display a notice informing you of the vulnerability fixes.



### `upgrade [packages...]` (alias: `up`)
Updates project dependencies to their latest available versions. By default, it respects semver ranges declared in your configuration file and updates both `dependencies` and `devDependencies`.

- `[packages...]`: Specific packages to upgrade. Alternatively, you can specify `"major"`, `"minor"`, or `"patch"` as package arguments to restrict the upgrade scope to only major, minor, or patch releases across dependencies. If omitted, all dependencies are checked.
- `-L, --latest`: Upgrade to the absolute latest version, ignoring semver ranges.
- `-F, --fixed`: Save the exact version resolved in the configuration file rather than prefixing it with a semver range (like `^`).
- `--prod`: Only upgrade `dependencies` (skip `devDependencies`).
- `--dev`: Only upgrade `devDependencies` (skip `dependencies`).
- `-n, --dry-run`: Show available updates without applying them.
- `--log`: Show detailed installation progress.

```bash
# Upgrade all dependencies within semver ranges
kumo upgrade

# Upgrade only patch releases for dependencies (e.g. 1.0.1 -> 1.0.2)
kumo upgrade patch

# Upgrade minor and patch releases for dependencies (e.g. 1.0.1 -> 1.1.2)
kumo upgrade minor

# Upgrade specific packages and write exact version numbers to package.json
kumo upgrade express typescript --fixed

# Upgrade to absolute latest versions (ignore semver ranges)
kumo upgrade --latest

# Only upgrade production dependencies
kumo upgrade --prod

# Preview available updates without changing anything
kumo upgrade --dry-run
```

### `ts`

#### `ts init`
Initializes a new TypeScript project by generating a default `tsconfig.json` file. It also creates a `.kumo/kumo.d.ts` declaration file in the project, automatically including it in the `tsconfig.json` `include` array. This registers types for the built-in global `Kumo` JavaScript API available when executing files via `kumo ts exec`.

```bash
kumo ts init
```

#### `ts build`
Transpiles TypeScript files. By default, it recursively transpiles all `.ts` files in the current project directory (excluding `node_modules` and hidden directories) to separate `.js` files in the output directory (`dist` by default).

Alternatively, you can bundle all compiled modules into a single, self-contained file in the output directory using the `--bundle` flag.

- `[file]`: Entry file or files/directories to build. If bundling, this is the entry point (defaults to `index.ts` or `src/index.ts`).
- `--bundle`: Bundles the output and all its local dependencies recursively into a single file.
- `--minify`: Minifies the output code.
- `--name <name>`: Custom output bundle file name (defaults to `bundle.js`).
- `--out <dir>`: Output directory for compiled files/bundles (defaults to `dist`).

```bash
# Compile everything separately in the 'dist/' folder (by default)
kumo ts build

# Compile and save results in a different output folder
kumo ts build src/index.ts --out build/

# Bundle and minify everything into a single file named "test.js" in the "dist/" folder
kumo ts build src/index.ts --bundle --minify --name test
```

#### `ts check`
Runs a type-check on the TypeScript project.

> [!WARNING]
> Native Rust-based type-checking is not currently supported. Executing `kumo ts check` will abort and advise you to use the official TypeScript compiler (`tsc --noEmit`) for type checking.

```bash
kumo ts check
```

#### `ts exec`
Executes a TypeScript file directly using Node.js combined with the native Rust transpiler loader.

```bash
kumo ts exec src/index.ts
```


#### Global Kumo API

When executing a TypeScript/JavaScript file using `kumo ts exec`, Kumo registers a global `Kumo` helper object containing utility methods for filesystem operations, child process execution, HTTP serving, and more:

- **`Kumo.version`**: The version string of the Kumo CLI.
- **`Kumo.env`**: Reference to `process.env`.
- **`Kumo.file(path)`**: Access file utilities for the given path:
  - `text()`: Promise returning file content as string.
  - `json<T>()`: Promise returning parsed JSON.
  - `exists()`: Synchronous check for file existence.
- **`Kumo.write(path, data)`**: Writes text or Uint8Array binary data to the file path.
- **`Kumo.spawn(command, args?, options?)`**: Helper to spawn child processes, returning a process wrapper.
- **`Kumo.sleep(ms)`**: Helper promise that resolves after the specified milliseconds.
- **`Kumo.serve(options)`**: Spawns a lightweight HTTP server (default port: `3000`) with a custom `fetch(request: Request): Promise<Response> | Response` request handler.
- **`Kumo.pkg.readConfig()`**: Parses and returns the project's local `kumo.json` configuration file. Note that it only checks for `kumo.json` and returns `null` if it is not present (it does not fall back to `package.json`).

Example usage:
```typescript
// Run with: kumo ts exec script.ts
console.log(`Running on Kumo v${Kumo.version}`);

if (Kumo.file("config.json").exists()) {
  const cfg = await Kumo.file("config.json").json();
  console.log("Config loaded:", cfg);
}

// Start a lightweight HTTP server
Kumo.serve({
  port: 8080,
  fetch: (req) => {
    return new Response(`Hello from Kumo server! Path: ${req.url}`);
  }
});
```

#### HTTPS Module Loader

Scripts run via `kumo ts exec` also benefit from a built-in HTTPS ESM loader. You can directly import packages from URLs (such as `esm.sh` or other CDNs) without installing them:

```typescript
import confetti from "https://esm.sh/canvas-confetti";
confetti();
```

*For security constraints (e.g. blocking HTTP or local connections), see the [Security documentation](security.md#8-secure-https-module-loader).*

### `config`
Manage Kumo configuration and security policies.

#### `config init`
Generates a default `kumo.config.json` file in the current directory.

```bash
kumo config init
```

#### `config default <setting> <value>`
Sets a global default configuration in `~/.kumo/kumo.config.json`. These values will be used across all your Kumo projects unless overridden by a local `kumo.config.json`.

```bash
# Enable Node.js module resolution natively for all projects
kumo config default useNodeModules true

# Disable blocking of deprecated packages globally
kumo config default block_deprecated false
```

### `update [version] [--pre]`
Checks for and installs the latest version (or a specified version) of the **Kumo CLI binary** from GitHub. This does not affect project dependencies — use `kumo upgrade` for that.

- `[version]`: A specific version to upgrade/downgrade to (e.g. `1.0.6`). Alternatively, you can specify one of `"alpha"`, `"beta"`, or `"rc"` to search for and update to the latest corresponding pre-release version.
- `--pre`: Includes pre-releases (alpha, beta, rc) in the update search (ignored if a specific version is provided).

```bash
kumo update
kumo update 1.0.6
kumo update --pre
kumo update alpha
```

### `run [script] [args...]`
Runs a script defined in `package.json` or `kumo.json` or a binary in the `.bin` directory. If no script name is specified, Kumo launches an interactive selector in the terminal to let you choose which script to run.

```bash
kumo run
kumo run build -- --production
```

### `auth [--registry <URL>]`
Authenticates with the Kumo registry. It generates a local cryptographic key pair (`private_key.pem` and `public_key.pem` inside `~/.kumo/`) and initiates a browser-based interactive OIDC login session.

- `--registry`: Custom registry URL to authenticate with. Note that this command is **strictly restricted** to the official Kumo registry (`https://kumo.unsetsoft.com`).

```bash
kumo auth
```

### `deps publish [path] [--registry <URL>]`
Publishes a package to the Kumo registry. It packs the target directory into a `.tgz` tarball, signs the version's BLAKE3 integrity checksum using your private key, and submits the package to the registry.

- `[path]`: The path to the package directory to publish. Defaults to the current directory (`.`).
- `--registry`: Custom registry URL to publish to. Note that this command is **strictly restricted** to the official Kumo registry (`https://kumo.unsetsoft.com`).

```bash
kumo deps publish
```

### `audit-fix`
Scans dependencies in your `kumo.lock` file for vulnerabilities. For any vulnerable packages, it checks the registry to see if a newer version is available that is free of known vulnerabilities. If a fix is found, Kumo automatically updates the semver ranges in `kumo.json` or `package.json` and advises running `kumo install` to apply the upgrade.

```bash
kumo audit-fix
```

### `shield [on | off | status]`
Manages Kumo Shield status. Kumo Shield leverages native OS attributes to mark global cache package files and critical local configurations (like `kumo.lock` and `kumo.config.json`) as Read-Only, preventing malicious scripts or processes from tampering with them in the background.

```bash
# Activate Kumo Shield
kumo shield on

# Check current Shield status
kumo shield status
```

### `unlock <file>`
Unlocks a shielded configuration file (`kumo.config.json`) or lockfile (`kumo.lock`) to allow manual editing.
> [!CAUTION]
> **Anti-Malware Trap:** This command is strictly gated behind an interactive terminal (TTY) check. Any script attempting to run this command non-interactively or piping input into it will be blocked.

```bash
kumo unlock kumo.config.json
```

### `lock [file]`
Manually re-locks files under Kumo Shield. If no file is specified, it defaults to locking both `kumo.config.json` and `kumo.lock`.

```bash
kumo lock
```

