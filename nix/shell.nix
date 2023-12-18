{
  perSystem =
    { config
    , self'
    , inputs'
    , pkgs
    , ...
    }:
    {
      devShells.default = pkgs.mkShell {
        packages = [
          inputs'.nixos-anywhere.packages.nixos-anywhere

          # code formatting
          config.treefmt.build.wrapper

          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # crane does not have this in nativeBuildInputs
          pkgs.nightlyToolchain
        ] ++ config.packages.kld.nativeBuildInputs
        ++ config.packages.kld.tests.nativeBuildInputs;
        inherit (config.packages.kld) buildInputs;
        RUST_BACKTRACE = 1;
        inherit (self'.packages.kld) nativeBuildInputs;
      };
    };
}
