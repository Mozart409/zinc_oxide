set dotenv-load

default:
    bacon run-long

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
