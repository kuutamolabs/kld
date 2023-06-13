{
  perSystem =
    { config
    , self'
    , pkgs
    , ...
    }:
    {
      devShells.default = pkgs.mkShell {
        packages = [
          # code formatting
          config.treefmt.build.wrapper

          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # rust dev
          pkgs.rust-analyzer
          pkgs.cargo-watch
          pkgs.clippy

          # crane does not have this in nativeBuildInputs
          pkgs.rustc
          pkgs.rust-analyzer
        ] ++ config.packages.kld.nativeBuildInputs
        ++ config.packages.kld.tests.nativeBuildInputs;
        inherit (config.packages.kld) buildInputs;
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE = 1;
        inherit (self'.packages.kld) nativeBuildInputs;
      };
    };
}
