{ self, ... }: {
  perSystem =
    { config
    , self'
    , inputs'
    , pkgs
    , ...
    }:
    let
      formatters = [
        pkgs.treefmt
        pkgs.nixpkgs-fmt

        # rust
        pkgs.clippy
        pkgs.rustfmt
      ];
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs =
          formatters ++ [
            # tasks and automation
            pkgs.just
            pkgs.jq
            pkgs.nix-update

            # rust dev
            pkgs.rust-analyzer
            pkgs.cargo-watch
          ]
          ++ self'.packages.lightning-knd.buildInputs;
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE = 1;
        nativeBuildInputs = self'.packages.lightning-knd.nativeBuildInputs;
        passthru = {
          inherit formatters;
        };
      };
    };
}
