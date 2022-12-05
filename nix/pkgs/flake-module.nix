{
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages = {
      lightning-knd = pkgs.callPackage ./lightning-knd.nix { };
      bitcoind = pkgs.bitcoind.override { withWallet = false; withGui = false; };
      cockroach = pkgs.callPackage ./cockroach.nix { };
      default = self'.packages.lightning-knd;
    };
  };
}
