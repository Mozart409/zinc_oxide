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
    cargo clippy --all-targets --all-features -- -D warnings
    cargo clippy --all-targets -- -D warnings
    dprint check

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
