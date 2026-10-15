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

          # The browser build, as a package: the same wasm `web/release.sh`
          # emits, next to the page it runs in. `buildRustPackage` is here only
          # because it knows how to vendor the crates for the network-less
          # sandbox; the build itself is one cargo invocation.
          webWasm = pkgs.rustPlatform.buildRustPackage {
            pname = "wycca-web";
            inherit version;
            src = wycca.src;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = false;
            # rustc shells out to `wasm-ld`, and nixpkgs' rustc ships the wasm32
            # std but not the linker that turns .o into a .wasm.
            nativeBuildInputs = [ pkgs.lld ];
            buildPhase = ''
              runHook preBuild
              cargo build --profile wasm-release -p wycca-core --target wasm32-unknown-unknown
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              install -Dm644 \
                target/wasm32-unknown-unknown/wasm-release/wycca_core.wasm \
                $out/wycca.wasm
              runHook postInstall
            '';
          };

          # The page itself: two tracked files out of `web/`, so the generated
          # wasm next to them is not dragged into the store with it.
          webFiles = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [ ./web/index.html ./web/ui.js ];
          };

          # A deployed page fetches its wasm like any other asset, so it has no
          # base64 fallback — the same two files minus one that `web/release.sh`
          # drops with a `grep`.
          web = pkgs.runCommand "wycca-web-${version}"
            {
              nativeBuildInputs = [ pkgs.binaryen ];
            } ''
              cp ${webWasm}/wycca.wasm .
              cp ${webFiles}/web/ui.js .
              grep -v 'wycca\.wasm\.js' ${webFiles}/web/index.html >index.html
              # Re-encode only; see the note in web/release.sh for why -Oz is off.
              wasm-opt \
                --enable-bulk-memory --enable-bulk-memory-opt \
                --enable-nontrapping-float-to-int --enable-multivalue \
                --enable-reference-types --strip-debug --strip-producers \
                wycca.wasm -o wycca.wasm.tmp && mv wycca.wasm.tmp wycca.wasm
              mkdir -p $out
              cp index.html ui.js wycca.wasm $out/
            '';
        in
        {
          packages = {
            default = wycca;
            inherit wycca;
            inherit web;
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
            # wasm32-unknown-unknown std ships with nixpkgs rustc, but the linker
            # that turns .o into a .wasm does not: rustc looks for `wasm-ld` on
            # PATH and this is the only place it comes from. Neither does the
            # re-encoder `web/release.sh` runs after it, which is the other half of
            # the browser build: binaryen's wasm-opt, with the default feature set
            # spelled out because it assumes almost nothing.
            pkgs.lld
            pkgs.binaryen
          ];
            # resvg's fontdb has nothing to scan in a nix shell; hand it a font file
            # explicitly instead of depending on fontconfig paths.
            WYCCA_FONT = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf";
          };
        };
    };
}