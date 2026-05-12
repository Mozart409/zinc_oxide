# AGENTS.md - Zinc Oxide Project Guide

This document provides essential information for AI coding agents working on the zinc_oxide repository.

## Project Overview

**zinc_oxide** is a Rust CLI tool that recursively searches for git repositories and reports their status (uncommitted changes, file counts, etc.). With the optional `nix` feature, it also checks discovered Nix flakes for available `flake.lock` updates without mutating the existing lock files.

- **Language**: Rust (Edition 2024)
- **Repository**: https://github.com/Mozart409/zinc_oxide
- **License**: MIT

## Cargo Features

- `nix` (off by default): Enables flake discovery and lock-update checking via the `-F` / `--flakes` flag. Requires the `nix` CLI to be available on `PATH` at runtime. When this feature is disabled, passing `--flakes` returns an error explaining that the feature must be enabled at build time.

## Build, Test, and Lint Commands

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Build with the optional Nix flake checker enabled
cargo build --features nix

# Run the flake checker against a path (requires `nix` on PATH)
cargo run --features nix -- -F -p ~/code

# Build with just
just                    # Runs bacon in watch mode
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test by name
cargo test test_find_git_repositories_empty_directory

# Run tests via just
just test

# Run tests in watch mode (via bacon)
bacon test

# Run specific test with bacon
bacon test -- test_name_here
```

### Linting and Formatting

```bash
# Run clippy
cargo clippy
cargo clippy --all-targets

# Format code with dprint
dprint fmt

# Check formatting
dprint check

# Run cargo deny (license and security audit)
cargo deny check
just deny
```

### Development Tools

```bash
# Watch mode for development (recommended)
bacon                   # Default: runs check
bacon run-long          # Run the CLI and restart on changes
bacon test              # Run tests in watch mode
bacon clippy-all        # Run clippy on all targets

# Available via nix dev shell
nix develop             # Enter development environment
```

## Code Style Guidelines

### Import Ordering

- Standard library imports first (e.g., `use std::{env, fs, path::PathBuf};`)
- External crate imports second (e.g., `use git2::{Repository, StatusOptions};`)
- Internal module imports last (if any)

### Naming Conventions

- **Functions/Variables**: `snake_case` (e.g., `find_git_repositories`, `repo_statuses`)
- **Structs/Enums**: `PascalCase` (e.g., `RepoStatus`, `Args`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `VERSION`)
- **Type aliases**: Use descriptive names that explain the type's purpose

### Error Handling

- Use `color_eyre::eyre::Result<T>` for fallible functions
- Use the `?` operator to propagate errors
- Handle errors gracefully - skip directories/files that can't be read rather than panicking
- For CLI errors, print to stderr: `eprintln!("Error: {e}")`

### Types

- Prefer explicit types over inference for public APIs
- Use `PathBuf` for path handling
- Use `&str` for string slices, `String` for owned strings
- Leverage Rust's type system with `Option` and `Result`

### Code Organization

- Main logic in `src/main.rs` (single-file binary)
- CLI arguments defined with `gumdrop::Options` derive macro
- Unit tests in `#[cfg(test)]` module within `main.rs`
- Integration tests in `tests/` directory

### Comments and Documentation

- Document public functions with `///` doc comments
- Use inline comments (`//`) sparingly, only for complex logic
- Keep comments up-to-date with code changes

## Project Structure

```
.
├── Cargo.toml          # Rust package manifest
├── src/
│   └── main.rs         # Main application code + unit tests
├── tests/
│   ├── integration_tests.rs   # CLI integration tests
│   ├── edge_cases.rs          # Edge case tests
│   └── run_function_tests.rs # Function-specific tests
├── justfile            # Task runner configuration
├── bacon.toml          # File watcher configuration
├── deny.toml           # Cargo deny configuration
├── dprint.json         # Code formatter configuration
├── cog.toml            # Conventional commits config
├── flake.nix           # Nix development environment
└── website/            # Separate web project (excluded from Rust build)
```

## Testing Strategy

### Unit Tests (in `src/main.rs`)

- Test individual functions in isolation
- Use `tempfile::TempDir` for temporary test directories
- Test edge cases like empty directories, nested repos, hidden dirs

### Integration Tests (in `tests/`)

- Test CLI behavior end-to-end using `assert_cmd`
- Use `cargo_bin_cmd!` macro to run the compiled binary
- Test command-line arguments and flags
- Create real git repositories using `git2` library for realistic tests

### Test Naming

- Prefix with `test_` followed by descriptive name
- Use `snake_case` for test names
- Include scenario in name: `test_<function>_<scenario>`

## Dependencies

### Production

- `color-eyre`: Error handling and reporting
- `git2`: Git operations (with `vendored-libgit2` feature)
- `gumdrop`: CLI argument parsing

### Development

- `assert_cmd`: CLI testing
- `predicates`: Assertion helpers for tests
- `tempfile`: Temporary directories for tests

## CI/CD

GitHub Actions workflow (`.github/workflows/rust.yml`):

1. Builds release binary
2. Runs all tests
3. Creates deb and rpm packages

## Git Workflow

- Follow conventional commits (enforced by cocogitto)
- Use `lefthook` for git hooks management
  - `pre-commit`: runs `keep-sorted`, `dprint check`, `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`, and `cargo test` in parallel
  - `pre-push`: runs `cargo deny check` and `cargo build --release --features nix` in parallel
- Main branch: `main`

## Important Notes

- **Git2 vendored**: The project uses vendored libgit2 to avoid system dependency issues
- **Hidden directories**: The code intentionally skips hidden directories (starting with `.`) during recursion
- **Graceful errors**: Permission denied and other IO errors are handled gracefully - directories are skipped rather than causing panics
- **Bare repositories**: The tool skips bare git repositories
- **Non-mutating flake checks**: `check_flake_updates` invokes `nix flake update --flake <path>` but redirects the output lock to a temporary path (`temporary_lock_path`), so the project's real `flake.lock` is never written. Flakes lacking a `flake.lock` are reported as needing initialization rather than being silently created.
- **Feature-gated code**: All flake logic is gated behind `#[cfg(feature = "nix")]`. A stub `collect_flake_statuses` exists under `#[cfg(not(feature = "nix"))]` that errors when `--flakes` is passed without the feature compiled in.
