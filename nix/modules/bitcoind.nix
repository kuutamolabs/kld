{ config
, lib
, ...
}:
let
  cfg = config.kuutamo.bitcoind;
  bitcoincfg = config.services.bitcoind.${cfg.instanceName};
in
{
  imports = [
    ./bitcoind-disks.nix
  ];

  options.kuutamo.bitcoind = {
    network = lib.mkOption {
      type = lib.types.enum [ "main" "testnet" "regtest" ];
      default = "main";
      description = "Bitcoin network to use.";
    };
    instanceName = lib.mkOption {
      type = lib.types.str;
      default = "kld-${cfg.network}";
      description = "Bitcoin network to use.";
    };
    package = lib.mkOption {
      type = lib.types.package;
      description = "The Bitcoind package to use for running the service.";
    };
  };

  config = {

    services.bitcoind.${cfg.instanceName} = {
      enable = true;
      inherit (cfg) package;
      testnet = cfg.network == "testnet";
      port =
        if cfg.network == "regtest" then
          18444
        else if cfg.network == "testnet" then
          18333
        else 8333;
      rpc.port = 8332;
      extraCmdlineOptions = lib.optionals (cfg.network == "regtest") [
        "-regtest"
        "-noconnect"
      ];
    };

    kuutamo.disko.bitcoindDataDir = bitcoincfg.dataDir;
  };
}
