# KX: Kumo Execute

`kx` (Kumo Execute) is a tool designed to run binaries from your project's local dependencies or to download and run packages temporarily, similar to `npx`.

## Usage

```bash
kx <binary> [args...]
```

## Features

- **Local Execution**: Checks for the binary in `dependencies/.bin` or `node_modules/.bin` and runs it if found. Kumo automatically detects which directory is being used in your project.
- **Auto-Install**: If the binary is not found locally, `kx` will ask if you want to install it temporarily. If confirmed, it will fetch the package from the registry, link its dependencies, and execute the binary without cluttering your project's main dependency list.
- **Path Integration**: Automatically adds the local `.bin` directory to the `PATH` during execution.

## Examples

Run a locally installed tool:
```bash
kx tsc --version
```

Run a tool without installing it permanently:
```bash
kx cowsay "Hello Kumo!"
```
