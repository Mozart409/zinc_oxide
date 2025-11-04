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
          nodejs_24
          nodePackages.pnpm
          ni
          bun
          docker
          docker-compose
          lazydocker
          caddy
          rust
          cargo
        ];

        shellHook = ''
          echo "node: $(node -v)"
          echo "bun: $(bun -v)"
          echo "To install dependencies, run: ni or bun install"

          # Add user to docker group if not already added
          if ! groups $USER | grep -q docker; then
            echo "Note: You may need to add your user to the docker group:"
            echo "sudo usermod -aG docker $USER"
            echo "Then log out and back in, or run: newgrp docker"
          fi
        '';
      };

      packages.default = pkgs.writeShellScriptBin "setup-project" ''
        pnpm install
      '';
    });
}
