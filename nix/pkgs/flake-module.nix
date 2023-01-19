{
  perSystem = { self', pkgs, ... }: {
    packages = {
      lightning-knd = pkgs.callPackage ./lightning-knd.nix { };
      bitcoind = pkgs.bitcoind.override { withWallet = false; withGui = false; };
      default = self'.packages.lightning-knd;
    };
  };
}
