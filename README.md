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

This recursively discovers directories containing a `flake.nix` and checks each one for available `flake.lock` updates. The check runs `nix flake update` against a throwaway lock path, so your existing `flake.lock` files are never modified. Flakes are checked in parallel, and each check is abandoned after `--flake-timeout` seconds (default 120). Flakes without a `flake.lock` are reported as needing initialization, and failed checks show the error from `nix`.

Add `-f` to list which inputs would change:

```bash
zinc_oxide -F -f -p ~/code/rust/axon-gateway
```

```
Found 1 Nix flake:

--- Flake: /home/you/code/rust/axon-gateway ---
Updates available!
  nixpkgs: 3b9f00d -> 8a2c4e1
```

## Options

- `-h, --help`: Print help message
- `-p, --path <p>`: Check this absolute path (defaults to current directory)
- `-f, --files`: Show individual files with uncommitted changes
- `-e, --empty`: Show empty repositories (those with no uncommitted changes)
- `-c, --compact`: Only print counts
- `-F, --flakes`: Check Nix flakes for lock updates, when built with `--features nix` (with `-f`, also lists changed inputs)
- `--flake-timeout <SECS>`: Seconds to wait for each flake check (default 120)

Hidden directories and symlinks are not followed while searching. zinc_oxide exits with a non-zero status on errors.

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
