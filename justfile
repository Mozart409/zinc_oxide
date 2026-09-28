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

deb: clear
    cargo deb

rpm: clear
    cargo generate-rpm

update:
    nix flake update
    git add flake.lock
    git commit --only flake.lock -m "chore: update flake.lock"
