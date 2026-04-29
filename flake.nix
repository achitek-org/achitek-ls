{
  description = "Achitek-ls Development Environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  inputs.crane.url = "github:ipetkov/crane";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  inputs.nil.url = "github:oxalica/nil/c8e8ce72442a164d89d3fdeaae0bcc405f8c015a";

  inputs.nil.flake = true;

  outputs =
    {
      self,
      crane,
      nil,
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;
        achitekLsCrate = craneLib.crateNameFromCargoToml {
          cargoToml = ./Cargo.toml;
        };

        nix-lsp-server = nil.packages.${system}.nil;

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;

          pname = "achitek-ls";
          inherit (achitekLsCrate) version;
          strictDeps = true;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];
        };

        cargoArtifacts = craneLib.buildDepsOnly (
          commonArgs
          // {
            pname = "achitek-ls";
          }
        );

        achitek-ls-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets --all-features -- --deny warnings";
          }
        );

        achitek-ls-test = craneLib.cargoNextest (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = "--workspace --all-features";
          }
        );

        achitek-ls-fmt = craneLib.cargoFmt {
          src = commonArgs.src;
        };

        achitek-ls = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--bin achitek-ls";
          }
        );
      in
      {
        packages = {
          default = achitek-ls;
          achitek-ls = achitek-ls;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = achitek-ls;
            name = "achitek-ls";
          };
          achitek-ls = flake-utils.lib.mkApp {
            drv = achitek-ls;
            name = "achitek-ls";
          };
        };

        checks = {
          inherit
            achitek-ls
            achitek-ls-clippy
            achitek-ls-fmt
            achitek-ls-test
            ;

          default = achitek-ls;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            achitek-ls
            cargo-dist
            cargo-nextest
            cargo-watch
            just
            natscli
            nix-lsp-server
            openssl
            pkg-config # needed by openssl to locate headers and libraries
            rust-analyzer
            lefthook
          ];

          shellHook = ''
            if [ ! -f .git/hooks/pre-commit ]; then
              lefthook install
            fi
          '';
        };
      }
    );
}
