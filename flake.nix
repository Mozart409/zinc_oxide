{
  description = "Development environment for a Node.js project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    wrangler-flake.url = "github:ryand56/wrangler";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    wrangler-flake,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
        overlays = [rust-overlay.overlays.default];
      };
      rust = pkgs.rust-bin.stable."1.92.0".default;
    in {
      # to use other shells, run:
      # nix develop . --command fish
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rust
          lazydocker
          opencode
          openssl
          pkg-config
          bacon
          cargo-deny
          lefthook
          cocogitto
          just
          pnpm
          nodejs_24
          ni
          nix-ld
          autoPatchelfHook
          # wrangler
          wrangler-flake.packages.${system}.wrangler
        ];

        shellHook = ''
          export LD_LIBRARY_PATH=${pkgs.nix-ld}/lib:$LD_LIBRARY_PATH
          export NIX_LD=${pkgs.glibc}/lib/ld-linux-x86-64.so.2
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
          ./patch-workerd.sh
          echo "Development environment is ready!"

          cargo -V
        '';
      };

      packages.default = pkgs.writeShellScriptBin "setup-project" ''
        cargo build
      '';
    });
}
