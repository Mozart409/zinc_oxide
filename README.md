# Zinc Oxide

A Rust CLI tool that recursively checks for git projects and reports their statuses.

## Description

Zinc Oxide scans directories to find git repositories and displays their current status, showing which repositories have uncommitted changes. It's useful for managing multiple projects and ensuring all your repositories are in a clean state.

## Installation

```bash
curl to bash
```

## Usage

### Basic usage

```bash
zinc_oxide
```

This will search the current directory recursively for git repositories and report their status.

### Specify a path

```bash
zinc_oxide --path /path/to/search
zinc_oxide -p ~/code/
```

### Show individual files

```bash
zinc_oxide -f
```

This will show the specific files that have uncommitted changes.

### Show empty repositories

```bash
zinc_oxide -e
```

This will include repositories that have no uncommitted changes in the output.

### Combine options

```bash
zinc_oxide --path ~/code -f -e
```

### Check Nix flake locks

The Nix flake lock checker is behind the optional `nix` feature:

```bash
cargo install --path . --features nix
zinc_oxide --path ~/code --flakes
```

This checks discovered Nix flakes for possible `flake.lock` updates without modifying the existing lock files.

## Options

- `-h, --help`: Print help message
- `-p, --path <p>`: Check this absolute path (defaults to current directory)
- `-f, --files`: Show individual files with uncommitted changes
- `-e, --empty`: Show empty repositories (those with no uncommitted changes)
- `-F, --flakes`: Check Nix flakes for lock updates, when built with `--features nix`

## Examples

```bash
# Search current directory for git repos with changes
zinc_oxide

# Search a specific directory and show changed files
zinc_oxide --path ~/dev -f

# Show all repositories including clean ones
zinc_oxide -e
```

## License

MIT
