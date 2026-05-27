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
Shows statistics about the Kumo store, including the number of unique packages, unique files, and disk usage.

```bash
kumo stats
```

### `prune`
Maintenance command to remove unnecessary files.

#### `prune cache [--full]`
Removes orphaned files from the global store.
- `--full`: Wipes the entire global store (all metadata and objects). Use this for a complete reset of the cache.

#### `prune deps [--full]`
Cleans the local dependencies directory.
- `--full`: Removes the entire directory (e.g., `node_modules/` or `dependencies/`) and the `kumo.lock` file.

```bash
kumo prune cache [--full]
kumo prune deps [--full]
```

### `doctor` (alias: `dr`)
Runs a health check on the store to detect and report corrupted files.

```bash
kumo doctor
```

### `explain <name>` (alias: `ex`)
Explains why a package is present in the dependency tree by showing which other packages depend on it.

```bash
kumo explain <package-name>
```

### `workspaces`
Detects and lists local packages in a monorepo structure.

```bash
kumo workspaces
```

### `patch <name>`
Extracts a package to `.kumo/patch/<name>` for manual patching. After editing, you can apply the changes.

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

### `upgrade [packages...]` (alias: `up`)
Updates project dependencies to their latest available versions. By default, it respects semver ranges declared in your configuration file and updates both `dependencies` and `devDependencies`.

- `[packages...]`: Specific packages to upgrade. If omitted, all dependencies are checked.
- `-L, --latest`: Upgrade to the absolute latest version, ignoring semver ranges.
- `--prod`: Only upgrade `dependencies` (skip `devDependencies`).
- `--dev`: Only upgrade `devDependencies` (skip `dependencies`).
- `-n, --dry-run`: Show available updates without applying them.
- `--log`: Show detailed installation progress.

```bash
# Upgrade all dependencies within semver ranges
kumo upgrade

# Upgrade specific packages
kumo upgrade express typescript

# Upgrade to absolute latest versions (ignore semver ranges)
kumo upgrade --latest

# Only upgrade production dependencies
kumo upgrade --prod

# Preview available updates without changing anything
kumo upgrade --dry-run
```

### `ts` (alias: `tsx`)
Provides a built-in TypeScript execution environment without requiring local dependencies. It automatically downloads the necessary compilers via Kumo Execute (`kx`) in the background.

#### `ts init`
Initializes a new TypeScript project by generating a default `tsconfig.json` file (runs `tsc --init`).

```bash
kumo ts init
```

#### `ts build`
Runs the official TypeScript compiler (`tsc`) on your project.

```bash
kumo ts build src/index.ts --noEmit
```
_For configuration options, see the [tsc documentation](https://www.typescriptlang.org/docs/handbook/compiler-options.html)._

#### `ts exec`
Executes a TypeScript file directly using `tsx` (TypeScript Execute).

```bash
kumo ts exec src/index.ts
```
_For execution options, see the [tsx documentation](https://tsx.hirok.io/getting-started)._

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

### `update [--pre]`
Checks for and installs the latest version of the **Kumo CLI binary** from GitHub. This does not affect project dependencies — use `kumo upgrade` for that.
- `--pre`: Includes pre-releases (alpha, beta, rc) in the update search.

```bash
kumo update
kumo update --pre
```
