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
      }: let
        wrangler = inputs'.wrangler-flake.packages.wrangler;
      in {
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
            cargo-deb
            cargo-deny
            cargo-edit
            cargo-workspaces
            claude-code
            cocogitto
            dprint
            just
            keep-sorted
            lazydocker
            lefthook
            ni
            nix-ld
            nodejs_24
            opencode
            pnpm
            watchexec
            # wrangler
            wrangler
            # keep-sorted end
          ];

          # The workerd binary npm installs into website/node_modules is linked
          # for a generic Linux and won't run on NixOS. Point miniflare (used by
          # `wrangler dev` and vitest-pool-workers) at the one the Nix wrangler
          # package ships, which is already patched.
          # miniflare only works with the workerd release it was built for, so
          # keep website's wrangler pinned to the version of ${wrangler}.
          # These versions drift: `nix flake update` bumps the Nix wrangler
          # while website/pnpm-lock.yaml stays put, and `wrangler dev` then
          # misbehaves (e.g. 404 on `/`). After updating, run
          # `pnpm add -D wrangler@<new nix version>` in website/.
          # @cloudflare/vitest-pool-workers bundles its own miniflare and can
          # drift the same way.
          MINIFLARE_WORKERD_PATH = "${wrangler}/lib/node_modules/workerd/bin/workerd";

          # website/package.json pins an older pnpm via `packageManager`; pnpm
          # would switch to a downloaded, generic-Linux build that can't run on
          # NixOS. Use the Nix pnpm instead.
          pnpm_config_manage_package_manager_versions = "false";

          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.nix-ld}/lib:$LD_LIBRARY_PATH
            export NIX_LD=${pkgs.glibc}/lib/ld-linux-x86-64.so.2
            echo "Development environment is ready!"

            cargo -V

            lefthook install
          '';
        };
      };
    };
}
