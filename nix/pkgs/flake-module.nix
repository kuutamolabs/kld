{
  perSystem = { config, self', pkgs, ... }: {
    packages = {
      lightning-knd = pkgs.callPackage ./lightning-knd.nix {
        inherit (config.packages) cockroachdb;
      };
      bitcoind = pkgs.bitcoind.override { withWallet = false; withGui = false; };
      cockroachdb = pkgs.callPackage ./cockroachdb.nix { };
      default = self'.packages.lightning-knd;
    };
  };
}
