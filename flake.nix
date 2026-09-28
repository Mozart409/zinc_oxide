{
  description = "Development environment for zinc_oxide (Rust CLI tool)";

  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    wrangler-flake.url = "github:ryand56/wrangler";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem = {
        pkgs,
        system,
        inputs',
        ...
      }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [inputs.fenix.overlays.default];
          config.allowUnfree = true;
        };

        # to use other shells, run:
        # nix develop . --command fish
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # keep-sorted start
            (fenix.complete.withComponents [
              "cargo"
              "clippy"
              "rustc"
              "rustfmt"
            ])
            autoPatchelfHook
            cargo-audit
            cargo-deny
            cargo-workspaces
            claude-code
            cocogitto
            # wrangler
            inputs'.wrangler-flake.packages.wrangler
            just
            keep-sorted
            lazydocker
            lefthook
            ni
            nix-ld
            nodejs_24
            opencode
            pnpm
            # keep-sorted end
          ];

          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.nix-ld}/lib:$LD_LIBRARY_PATH
            export NIX_LD=${pkgs.glibc}/lib/ld-linux-x86-64.so.2
            ./patch-workerd.sh
            echo "Development environment is ready!"

            cargo -V

            lefthook install
          '';
        };
      };
    };
}
