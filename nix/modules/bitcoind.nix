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
      type = lib.types.enum [ "main" "testnet" "regtest" "mutinynet" ];
      default = "main";
      description = "Bitcoin network to use.";
    };
    instanceName = lib.mkOption {
      type = lib.types.str;
      default = if cfg.network == "mutinynet" then "kld-signet" else "kld-${cfg.network}";
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
      ] ++ lib.optionals (cfg.network == "mutinynet") [
        "-signetchallenge=512102f7561d208dd9ae99bf497273e16f389bdbd6c4742ddb8e6b216e64fa2928ad8f51ae"
        "-addnode=45.79.52.207:38333"
        "-dnsseed=0"
        "-signetblocktime=30"
      ];
    };

    kuutamo.disko.bitcoindDataDir = bitcoincfg.dataDir;
  };
}
