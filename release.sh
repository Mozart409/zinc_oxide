#!/usr/bin/env bash
set -euo pipefail

# Get version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="v${VERSION}"

echo "Preparing release ${TAG}"

# Must be on main
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
    echo "Error: must be on main (currently on ${BRANCH})"
    exit 1
fi

# Tag must not already exist
if git rev-parse "${TAG}" >/dev/null 2>&1; then
    echo "Error: Tag ${TAG} already exists"
    exit 1
fi

# Working tree may only contain changes to Cargo.toml / Cargo.lock.
# Uses `git status --porcelain` so untracked files are caught too.
DIRTY=$(git status --porcelain)
if [ -n "$DIRTY" ]; then
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        path="${line:3}"
        case "$path" in
            Cargo.toml|Cargo.lock) ;;
            *)
                echo "Error: unexpected working-tree change in '${path}'"
                echo "Only Cargo.toml and Cargo.lock may differ at release time"
                exit 1
                ;;
        esac
    done <<< "$DIRTY"
fi

# Verify the release toolchain
echo "Running cargo test (default features)..."
cargo test

echo "Running cargo test --features nix..."
cargo test --features nix

echo "Running cargo clippy --all-features..."
cargo clippy --all-targets --all-features -- -D warnings

echo "Running cargo deny check..."
cargo deny check

# Stage and commit (conventional-commits format for cocogitto/lefthook)
echo "Committing release..."
git add Cargo.toml Cargo.lock
git commit -m "chore(release): ${TAG}"

# Create tag locally, then push commit + tag atomically so CI never sees
# the release commit without its tag.
echo "Creating tag ${TAG}..."
git tag "${TAG}" -m "release: ${TAG}"

echo "Pushing commit and tag to origin..."
git push --atomic origin main "${TAG}"

echo "Release ${TAG} complete!"
