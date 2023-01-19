{
  perSystem =
    { config
    , self'
    , pkgs
    , ...
    }:
    {
      devShells.default = pkgs.mkShell {
        buildInputs = [
          # code formatting
          config.packages.treefmt

          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # rust dev
          pkgs.rust-analyzer
          pkgs.cargo-watch
          pkgs.clippy
        ] ++ self'.packages.lightning-knd.buildInputs;
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE = 1;
        inherit (self'.packages.lightning-knd) nativeBuildInputs;
      };
    };
}
