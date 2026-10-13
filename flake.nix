{
  description = "wycca: circular calendar renderer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.flake-parts.flakeModules.devShells ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        { pkgs, lib, ... }:
        let
          version = "0.1.0";

          wycca = pkgs.rustPlatform.buildRustPackage {
            pname = "wycca";
            inherit version;
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
          };
        in
        {
          packages = {
            default = wycca;
            inherit wycca;
          };

          apps.default = {
            type = "app";
            program = "${wycca}/bin/wycca";
          };

devShells.default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
              # The spec's wallpaper story is `wycca --png && feh --bg-fill`.
              pkgs.feh
              # resvg finds no system fonts in a nix shell, so --png needs one.
              pkgs.dejavu_fonts
            ];
            # resvg's fontdb has nothing to scan in a nix shell; hand it a font file
            # explicitly instead of depending on fontconfig paths.
            WYCCA_FONT = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf";
          };
        };
    };
}