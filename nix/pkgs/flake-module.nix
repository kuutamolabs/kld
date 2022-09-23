{
  perSystem = { pkgs, ... }: {
    packages = {
      sensei = pkgs.callPackage ./sensei { };
      # Inspired by https://docs.lightning.engineering/lightning-network-tools/lnd/optimal-configuration-of-a-routing-node
      bitcoind = pkgs.bitcoind.override { withWallet = false; };
    };
  };
}
