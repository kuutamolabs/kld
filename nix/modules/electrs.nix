{ config, lib, pkgs, ... }:
let
  cfg = config.kuutamo.electrs;
  bitcoinCfg = if cfg.bitcoindInstance == "bitcoind" then config.services.bitcoind else config.services.bitcoind.${cfg.bitcoindInstance};
  bitcoinCookieDir =
    if cfg.network == "regtest" then
      "${bitcoinCfg.dataDir}/regtest"
    else if cfg.network == "testnet" then
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
    bitcoindInstance = lib.mkOption {
      type = lib.types.str;
      default = "kld-${if cfg.network == "bitcoin" then "main" else cfg.network}";
      description = "The instance of bitcoind";
    };
    monitoringPort = lib.mkOption {
      type = lib.types.port;
      default = 4224;
      description = "Prometheus monitoring port.";
    };
    network = lib.mkOption {
      type = lib.types.enum [ "bitcoin" "testnet" "signet" "regtest" ];
      default = "bitcoin";
      description = lib.mdDoc "Bitcoin network to use.";
    };
    logLevel = lib.mkOption {
      type = lib.types.enum [ "error" "warn" "info" "debug" "trace" ];
      default = "warn";
      description = "Log level for Electrs";
    };
  };
  config = {
    users.users.electrs = {
      isSystemUser = true;
      group = "electrs";
    };
    users.groups.electrs = { };

    systemd.services.electrs = lib.mkDefault {
      wantedBy = [ "multi-user.target" ];
      after = [ "bitcoind${if cfg.bitcoindInstance == "bitcoind" then "" else cfg.bitcoindInstance}.service" ];
      serviceConfig = {
        ExecStartPre = "+${pkgs.writeShellScript "setup" ''
          until [ -e ${bitcoinCookieDir}/.cookie ]
          do
            sleep 10
          done
          install -m400 -o electrs ${bitcoinCookieDir}/.cookie /var/lib/electrs/.cookie
        ''}";
        ExecStart = ''
          ${pkgs.electrs}/bin/electrs \
          --log-filters=${cfg.logLevel} \
          --network=${cfg.network} \
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
