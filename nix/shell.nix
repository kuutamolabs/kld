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
        # our meta-formatter
        pkgs.treefmt
        # nix
        pkgs.nixpkgs-fmt
        # rust
        pkgs.rustfmt
        pkgs.clippy
      ];
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs =
          formatters
          ++ [
            # tasks and automation
            pkgs.just
            pkgs.jq
            pkgs.nix-update

            # rust dev
            pkgs.rust-analyzer
            pkgs.cargo-watch
            pkgs.clippy
          ]
          ++ self'.packages.lightning-knd.buildInputs;
        nativeBuildInputs = self'.packages.lightning-knd.nativeBuildInputs;
        passthru = {
          inherit formatters;
        };
      };
    };
}
