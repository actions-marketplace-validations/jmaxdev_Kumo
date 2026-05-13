# Kumo CLI Commands

Kumo is a security-first, space-efficient package manager. This document covers the main `kumo` command and its subcommands.

## Usage

```bash
kumo [COMMAND] [OPTIONS]
```

## Dependency Management

Kumo uses a space-efficient approach to dependency management. By default, it stores packages in a global store located at `~/.kumo/store` and links them into your project.

### Dependency Directory Detection
Kumo automatically detects where to link your dependencies:
1. If a `node_modules` directory already exists, Kumo will use it.
2. Otherwise, it defaults to a `dependencies` directory.

> [!TIP]
> If Kumo creates a `dependencies` directory for the first time, it will automatically attempt to add it to your `.gitignore` file.

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

#### `prune cache`
Removes orphaned files from the global store.

#### `prune deps [--full]`
Cleans the local dependencies directory.
- `--full`: Also removes the `kumo.lock` file.

```bash
kumo prune cache
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
Generates a Mermaid-compatible dependency graph of the project.

```bash
kumo graph
```

### `sandbox <script>`
Executes a script within the Kumo Sandbox for secure execution.

```bash
kumo sandbox <script-path>
```

### `update`
Checks for and installs the latest version of the Kumo CLI from GitHub.

```bash
kumo update
```
