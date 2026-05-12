{
  description = "Development environment for zinc_oxide (Rust CLI tool)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    wrangler-flake.url = "github:ryand56/wrangler";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    wrangler-flake,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };
    in {
      # to use other shells, run:
      # nix develop . --command fish
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          # keep-sorted start
          autoPatchelfHook
          cargo
          cargo-audit
          cargo-deny
          cargo-workspaces
          claude-code
          clippy
          cocogitto
          just
          keep-sorted
          lazydocker
          lefthook
          ni
          nix-ld
          nodejs_24
          opencode
          openssl
          pkg-config
          pnpm
          rustc
          rustfmt
          # wrangler
          wrangler-flake.packages.${system}.wrangler
          # keep-sorted end
        ];

        shellHook = ''
          export LD_LIBRARY_PATH=${pkgs.nix-ld}/lib:$LD_LIBRARY_PATH
          export NIX_LD=${pkgs.glibc}/lib/ld-linux-x86-64.so.2
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
          ./patch-workerd.sh
          echo "Development environment is ready!"

          cargo -V

          cog install-hook
          lefthook install
        '';
      };
    });
}
