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
just                    # Pick a recipe interactively (just --choose)
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test by name
cargo test test_cli_default_report_matches_golden

# Regenerate golden files after an intended output change (review the diff!)
UPDATE_GOLDEN=1 cargo test

# Run tests via just
just test

# Run tests in watch mode (via watchexec)
watchexec -e rs,toml -- cargo test

# Run specific test in watch mode
watchexec -e rs,toml -- cargo test test_name_here
```

### Linting and Formatting

```bash
# Run all lints (clippy with and without features + dprint check); warnings are errors
just lint

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
watchexec -e rs,toml -- cargo check                # Type-check on changes
watchexec -r -e rs,toml -- cargo run -- -p ~/code  # Run the CLI and restart on changes
watchexec -e rs,toml -- cargo test                 # Run tests on changes
watchexec -e rs,toml -- just lint                  # Lint on changes

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
- CLI arguments defined with `clap::Parser` derive macro
- All tests are end-to-end tests in `tests/`; `src/main.rs` has no unit tests

### Lints

- Clippy lints are configured in `Cargo.toml` under `[lints]`: `pedantic` and `nursery` are denied, as are panicking constructs (`unwrap_used`, `expect_used`, `indexing_slicing`, `panic`, `as_conversions`, etc.). All warnings are errors.
- `clippy.toml` allows `unwrap`/`expect`/indexing/`panic` inside tests only.
- Always lint via `just lint`; the git hooks, CI and the `cog bump` pre-bump hooks call it.

### Comments and Documentation

- Document public functions with `///` doc comments
- Use inline comments (`//`) sparingly, only for complex logic
- Keep comments up-to-date with code changes

## Project Structure

```
.
├── Cargo.toml          # Rust package manifest
├── src/
│   └── main.rs         # Main application code
├── tests/
│   ├── cli.rs          # E2E git report tests against golden files
│   ├── flake_tests.rs  # E2E flake checker tests (`nix` feature, needs `nix` on PATH, offline)
│   ├── common/mod.rs   # Shared helpers: real git repos via git2, output normalization, golden files
│   └── golden/         # Expected normalized output per CLI mode
├── justfile            # Task runner configuration
├── deny.toml           # Cargo deny configuration
├── dprint.json         # Code formatter configuration
├── cog.toml            # Conventional commits config
├── flake.nix           # Nix development environment
└── website/            # Separate web project (excluded from Rust build); `nr test` runs its E2E tests against a local wrangler worker
```

## Testing Philosophy

- **NEVER write unit tests after you write code.** Unit tests written after the
  fact tend to just re-describe the implementation rather than verify
  behavior.
- **Highly prefer E2E tests as the sole testing mechanism.** Use them to
  verify complex features work end-to-end. At the end of an E2E test, produce
  a verifiable and repeatable artifact (e.g. a downloaded/verified file, a
  persisted DB row, an API response fixture) rather than just asserting a
  process exited cleanly.
- **If you must test a system in isolation**, first write down all the ways
  it could fail, _then_ write the code to guard against those failure modes.
  Do not write the code first and backfill unit tests against it.
- **When writing an E2E test, don't pick the simplest possible scenario to
  prove the happy path works.** Pick a medium-to-hard scenario when verifying
  the work.

## Testing Strategy

- Every test runs the compiled binary (`cargo_bin_cmd!` from `assert_cmd`) against a workspace built in a `tempfile::TempDir`.
- **Git report (`tests/cli.rs`)**: `build_workspace` creates one realistic tree covering every rule (dirty/clean/fresh repos, a repo nested in a repo, bare `.git`, invalid `.git`, hidden, unreadable and symlinked dirs, deep nesting, unusual names). The full stdout of each mode is normalized (`<ROOT>`, `<VERSION>`) and compared to `tests/golden/*.txt`. When a new behavior is added, extend the workspace rather than adding a new minimal scenario.
- **Flakes (`tests/flake_tests.rs`)**: real `nix` against local `git+file` inputs (no network); the artifact is the byte-for-byte unchanged `flake.lock` files. A fake hanging `nix` on `PATH` covers timeouts.
- Use real repositories made with `git2` (`tests/common`), never an empty `.git` directory: libgit2 can't open those, so the repo is silently skipped and the test proves nothing.
- Assert exact output (golden files or `assert_eq!` on whole sections), not just exit status or a loose `contains`.
- To check the suite still catches bugs, inject a bug into `src/main.rs` and confirm a test fails.

### Test Naming

- Prefix with `test_` followed by descriptive name
- Use `snake_case` for test names
- Include scenario in name: `test_<function>_<scenario>`

## Dependencies

### Production

- `color-eyre`: Error handling and reporting
- `git2`: Git operations (with `vendored-libgit2` feature)
- `clap` (derive): CLI argument parsing

### Development

- `assert_cmd`: CLI testing
- `predicates`: Assertion helpers for tests
- `tempfile`: Temporary directories for tests

## CI/CD

GitHub Actions workflow (`.github/workflows/rust.yml`):

1. Builds release binary
2. Runs all tests
3. Creates deb and rpm packages
4. Installs Nix and builds/tests with `--features nix` (separate `nix-feature` job)

## Git Workflow

- Follow conventional commits (enforced by cocogitto)
- Use `lefthook` for git hooks management
  - `pre-commit`: runs `keep-sorted` (auto-fixes and restages `*.nix`), `dprint fmt` followed by `just lint`, and `cargo test` in parallel
  - `commit-msg`: validates the commit message with `cog verify`
  - `pre-push`: runs `keep-sorted --mode=lint`, `cargo deny check`, `cargo build --release --features nix`, `just lint`, and the website tests (`nr test` in `website/`) in parallel
- Main branch: `main`
- Releases: run `just release` (`cog bump --auto`) on a clean `main`. Pre-bump hooks in `cog.toml` run tests (with and without `nix`), `just lint`, `cargo deny check`, then `cargo set-version` updates `Cargo.toml`/`Cargo.lock`; cog writes `CHANGELOG.md`, commits `chore(version): vX.Y.Z`, tags `vX.Y.Z`, and the post-bump hook pushes commit and tag atomically (the tag triggers `.github/workflows/release.yml`)

## Important Notes

- **Git2 vendored**: The project uses vendored libgit2 to avoid system dependency issues
- **Hidden directories**: The code intentionally skips hidden directories (starting with `.`) during recursion
- **Graceful errors**: Permission denied and other IO errors are handled gracefully - directories are skipped rather than causing panics
- **Bare and invalid repositories**: Repos that libgit2 cannot open or cannot compute a status for (including bare repos, whose status call errors) are skipped and not counted in "Found N git repositories"
- **Single walk, no symlinks**: `find_projects` collects git repos and (with `nix`) flakes in one pass, sorted by path. It uses `DirEntry::file_type`, so symlinks are never followed (no loops, no walking into `/nix/store` via `result` links).
- **Non-mutating flake checks**: `flake::check` invokes `nix flake update --flake <path> --output-lock-file <tempdir>/flake.lock`, so the project's real `flake.lock` is never written. `nix` prints nothing in this mode, so updates are detected by comparing the parsed old and new `nodes`; `-f` then lists the root's direct inputs, resolved by input name (not node key, since nix renumbers keys like `nixpkgs_2`), whose `locked` entry changed, or notes that only transitive inputs changed. Flakes lacking a `flake.lock` are reported as needing initialization rather than being silently created.
- **Parallel, bounded flake checks**: `flake::check_all` runs up to 8 checks at once; each `nix` process is killed after `--flake-timeout` seconds. Failures keep nix's stderr and are printed.
- **Feature-gated code**: All flake logic lives in `mod flake` gated behind `#[cfg(feature = "nix")]`, with `serde_json` and `tempfile` as optional dependencies of the feature. Without the feature, `run` errors when `--flakes` is passed.
- **Exit codes**: `main` returns `ExitCode::FAILURE` when `run` errors.

## Common Pitfalls

- **Nightly toolchain everywhere**: The dev shell (`flake.nix`, fenix `complete`) and CI (`dtolnay/rust-toolchain@nightly`) both use nightly Rust. Do not switch either one to stable. Clippy lint behaviour differs between versions, so a mismatch makes `just lint` pass locally and fail in CI. `just lint` refuses to run on a non-nightly `rustc`.
- **`unwrap` in test helpers**: `clippy.toml` allows `unwrap`/`expect`/`panic` only in test contexts, and some clippy versions do not count plain helper functions in `tests/*.rs` (no `#[test]`) as test code. Make helpers return `Result<(), Box<dyn Error>>`, use `?` inside them, and `.unwrap()` at the `#[test]` call site.
- **Toolchain drift**: CI installs the latest nightly, while the dev shell is pinned by `flake.lock`. If CI reports lints you can't reproduce, run `nix flake update` (or `just update`) to bring the local nightly up to date.
