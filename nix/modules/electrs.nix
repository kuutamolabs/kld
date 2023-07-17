{ config, lib, pkgs, ... }:
let
  cfg = config.kuutamo.electrs;
  inherit (config.kuutamo.kld) network;
  bitcoind-instance = "kld-${network}";
  bitcoinCfg = config.services.bitcoind.${bitcoind-instance};
  bitcoinCookieDir =
    if network == "regtest" then
      "${bitcoinCfg.dataDir}/regtest"
    else if network == "testnet" then
      "${bitcoinCfg.dataDir}/testnet3"
    else bitcoinCfg.dataDir;
in
{
  options.kuutamo.electrs = {
    address = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Address to listen for RPC connections.";
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 50001;
      description = "Port to listen for RPC connections.";
    };
    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/electrs";
      description = "The data directory for electrs.";
    };
    monitoringPort = lib.mkOption {
      type = lib.types.port;
      default = 4224;
      description = "Prometheus monitoring port.";
    };
  };
  config = {
    users.users.electrs = {
      isSystemUser = true;
      group = "electrs";
    };
    users.groups.electrs = { };

    systemd.services.electrs = {
      wantedBy = [ "multi-user.target" ];
      after = [ "bitcoind.service" ];
      serviceConfig = {
        ExecStartPre = "+${pkgs.writeShellScript "setup" ''
          install -m400 -o electrs ${bitcoinCookieDir}/.cookie /var/lib/electrs/.cookie
        ''}";
        ExecStart = ''
          ${pkgs.electrs}/bin/electrs \
          --log-filters=INFO \
          --network=${network} \
          --db-dir=${cfg.dataDir} \
          --cookie-file=/var/lib/electrs/.cookie \
          --electrum-rpc-addr=${cfg.address}:${toString cfg.port} \
          --monitoring-addr=${cfg.address}:${toString cfg.monitoringPort} \
          --daemon-dir='${bitcoinCfg.dataDir}' \
          --daemon-rpc-addr=127.0.0.1:${toString bitcoinCfg.rpc.port} \
          --daemon-p2p-addr=127.0.0.1:${toString bitcoinCfg.port} \
        '';
        User = "electrs";
        Group = "electrs";
        Restart = "on-failure";
        RestartSec = "10s";
        ReadWritePaths = [ cfg.dataDir ];
        StateDirectory = "electrs";
      };
    };
  };
}
