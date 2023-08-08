{ config
, lib
, ...
}:
let
  kld-cfg = config.kuutamo.kld;
  bitcoind-instance = "kld-${kld-cfg.network}";
  bitcoinCfg = config.services.bitcoind.${bitcoind-instance};
in
{
  imports = [
    ./bitcoind-disks.nix
  ];

  config = {

    services.bitcoind.${bitcoind-instance} = {
      enable = true;
      testnet = kld-cfg.network == "testnet";
      port =
        if kld-cfg.network == "regtest" then
          18444
        else if kld-cfg.network == "testnet" then
          18333
        else 8333;
      rpc.port = 8332;
      extraCmdlineOptions = lib.optionals (kld-cfg.network == "regtest") [
        "-regtest"
        "-noconnect"
      ];
    };

    kuutamo.disko.bitcoindDataDir = bitcoinCfg.dataDir;
  };
}
