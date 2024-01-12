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
    package = lib.mkOption {
      type = lib.types.package;
      description = "The Bitcoind package to use for running the service.";
    };
  };

  config = {

    services.bitcoind.${cfg.instanceName} = {
      enable = true;
      inherit (cfg) package;
      testnet = false;
      port = 8333;
      rpc.port = 8332;
      extraCmdlineOptions = [
        "-signetchallenge=512102f7561d208dd9ae99bf497273e16f389bdbd6c4742ddb8e6b216e64fa2928ad8f51ae"
        "-addnode=45.79.52.207:38333"
        "-dnsseed=0"
        "-signetblocktime=30"
      ];
    };

    kuutamo.disko.bitcoindDataDir = bitcoincfg.dataDir;
  };
}
