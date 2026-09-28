set dotenv-load
set unstable
default:
    just --choose

deny: clear
    cargo deny check

clear:
    clear

test: clear
    cargo test

lint:
    @if ! rustc -V | grep -q nightly; then echo "error: just lint must run on the nightly toolchain (matches CI); enter the dev shell with 'nix develop'" >&2; exit 1; fi
    cargo hack --feature-powerset clippy --all-targets -- -D warnings
    dprint check

# Build and test every combination of Cargo features, so none of them rots.
features:
    cargo hack --feature-powerset build --release
    cargo hack --feature-powerset test

# Build the flake package users install (`nix build`/`nix profile install`).
nix-build:
    nix build --no-link -L .#checks.$(nix eval --impure --raw --expr builtins.currentSystem).package

deb: clear
    cargo deb

rpm: clear
    cargo generate-rpm

release:
    cog bump --auto

update:
    nix flake update
    git add flake.lock
    git commit --only flake.lock -m "chore: update flake.lock"

# website's wrangler must match the Nix wrangler version (see flake.nix);
# they drift after `nix flake update`.
[doc('Run the website dev server')]
[working-directory('website')]
website:
    ni --frozen
    nr dev
