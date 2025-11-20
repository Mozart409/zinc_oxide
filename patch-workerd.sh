#!/usr/bin/env bash
# Patch dynamically linked binaries in node_modules for NixOS

echo "Patching workerd binary for NixOS..."

WORKERD_PATH="./website/node_modules/.pnpm/@cloudflare+workerd-linux-64@*/bin/workerd"

# Find the workerd binary
WORKERD_BIN=$(find ./website/node_modules -name "workerd" -type f -executable 2>/dev/null | head -1)

if [ -z "$WORKERD_BIN" ]; then
    echo "Could not find workerd binary"
    exit 1
fi

echo "Found workerd at: $WORKERD_BIN"

# Patch the binary using patchelf
if command -v patchelf >/dev/null 2>&1; then
    patchelf --set-interpreter "$(cat $NIX_CC/nix-support/dynamic-linker)" "$WORKERD_BIN"
    echo "Patched workerd binary"
else
    echo "patchelf not found, installing..."
    nix-shell -p patchelf --run "patchelf --set-interpreter \$(cat \$NIX_CC/nix-support/dynamic-linker) $WORKERD_BIN"
fi

echo "Done!"