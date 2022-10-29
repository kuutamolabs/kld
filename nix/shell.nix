{ self, ... }: {
  perSystem =
    { config
    , self'
    , inputs'
    , pkgs
    , ...
    }: {
      packages.treefmt = self.inputs.treefmt-nix.lib.mkWrapper pkgs {
        # Used to find the project root
        projectRootFile = "flake.lock";

        programs.nixpkgs-fmt.enable = true;
        programs.rustfmt.enable = true;
      };
      devShells.default = pkgs.mkShell {
        buildInputs = [
          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # check format
          self'.packages.treefmt

          # rust dev
          pkgs.rust-analyzer
          pkgs.cargo-watch
          pkgs.clippy

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
