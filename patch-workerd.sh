#!/usr/bin/env bash
# Patch dynamically linked binaries in node_modules for NixOS
set -euo pipefail

echo "Patching workerd binary for NixOS..."

mapfile -t WORKERD_BINS < <(find ./website/node_modules -name "workerd" -type f -executable 2>/dev/null)

if [ "${#WORKERD_BINS[@]}" -eq 0 ]; then
    echo "Could not find workerd binary"
    exit 1
fi

if ! command -v patchelf >/dev/null 2>&1; then
    echo "patchelf not found on PATH — enter the nix dev shell first (nix develop)"
    exit 1
fi

INTERPRETER=$(cat "$NIX_CC/nix-support/dynamic-linker")
patched=0

for bin in "${WORKERD_BINS[@]}"; do
    if ! file -b "$bin" 2>/dev/null | grep -q ELF; then
        echo "Skipping (not ELF): $bin"
        continue
    fi
    echo "Found workerd at: $bin"
    patchelf --set-interpreter "$INTERPRETER" "$bin"
    echo "Patched: $bin"
    patched=$((patched + 1))
done

if [ "$patched" -eq 0 ]; then
    echo "No ELF workerd binaries found to patch"
    exit 1
fi

echo "Done! Patched $patched binary(ies)."
