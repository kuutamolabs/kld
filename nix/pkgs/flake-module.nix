{
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages = {
      lightning-knd = pkgs.callPackage ./lightning-knd.nix { };
      # Inspired by https://docs.lightning.engineering/lightning-network-tools/lnd/optimal-configuration-of-a-routing-node
      bitcoind = pkgs.bitcoind.override { withWallet = false; };
    };
  };
}
