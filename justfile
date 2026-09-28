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
    cargo clippy --all-targets --all-features -- -D warnings
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
