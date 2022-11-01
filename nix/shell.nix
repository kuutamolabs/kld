{ self, ... }: {
  perSystem =
    { config
    , self'
    , inputs'
    , pkgs
    , ...
    }: {
      devShells.default = pkgs.mkShell {
        buildInputs = [
          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # check format
          pkgs.treefmt
          pkgs.nixpkgs-fmt

          # rust dev
          pkgs.rust-analyzer
          pkgs.cargo-watch
          pkgs.clippy
          pkgs.rustfmt

          # lightning-knd dependencies
          (pkgs.bitcoind.override { withWallet = false; withGui = false; })
          pkgs.minio
          pkgs.minio-certgen
        ]
        ++ self'.packages.lightning-knd.buildInputs;
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE = 1;
        nativeBuildInputs = self'.packages.lightning-knd.nativeBuildInputs;
      };
    };
}
