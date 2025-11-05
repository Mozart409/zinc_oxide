{
  description = "Development environment for a Node.js project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
    unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    unstable,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      unstablePkgs = import unstable {inherit system;};
    in {
      # to use other shells, run:
      # nix develop . --command fish
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          lazydocker
          bacon
          cargo-deny
          lefthook
          cocogitto
          just
        ];

        shellHook = ''
          echo "Development environment is ready!"
        '';
      };

      packages.default = pkgs.writeShellScriptBin "setup-project" ''
        cargo build
      '';
    });
}
