{ config
, lib
, pkgs
, ...
}:
let
  cfg = config.kuutamo.lightning-knd;
  bitcoind-instance = "lightning-knd-${cfg.network}";
  bitcoinCfg = config.services.bitcoind.${bitcoind-instance};
in
{
  options.kuutamo.lightning-knd = {
    nodeId = lib.mkOption {
      type = lib.types.str;
      default = config.networking.hostName;
      description = ''
        Node ID used in logs
      '';
    };
    package = lib.mkOption {
      type = lib.types.package;
      description = lib.mdDoc ''
        Lightning-knd package to use
      '';
    };
    logLevel = lib.mkOption {
      type = lib.types.enum [ "info" "debug" "trace" ];
      default = "info";
      example = "debug";
      description = lib.mdDoc "Log level for lightning-knd";
    };
    peerPort = lib.mkOption {
      type = lib.types.port;
      default = 9234;
      description = lib.mdDoc "Port to listen for lightning peer connections";
    };
    network = lib.mkOption {
      # Our bitcoind module does not handle anything but bitcoind and testnet at the moment.
      # We might however not need more than that.
      #type = lib.types.enum [ "bitcoin" "testnet" "signet" "regtest" ];
      type = lib.types.enum [ "main" "testnet" ];
      default = "main";
      description = lib.mdDoc "Bitcoin network to use.";
    };

    testnet = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = lib.mdDoc "Whether to use the testnet instead of mainnet.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = lib.mDoc ''
        Whether to open ports used by lightning-knd
      '';
    };
    publicAddresses = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = lib.mDoc ''
        Comma-seperated list of ip addresses on which the lightning is *directly* reachable.
      '';
    };
    exporterAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:2233";
      description = lib.mDoc ''
        Address and port to bind to for exporting metrics
      '';
    };
    restApiAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:2244";
      description = lib.mDoc ''
        Address and port to bind to for the REST API
      '';
    };

    nodeAlias = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        The alias of this lightning node
      '';
    };
  };

  config = {
    # for cli
    environment.systemPackages = [ cfg.package ];

    services.cockroachdb.ensureDatabases = [ "lightning_knd" ];
    services.cockroachdb.ensureUsers = [{
      name = "lightning-knd";
      ensurePermissions."DATABASE lightning_knd" = "ALL";
    }];

    services.bitcoind.${bitcoind-instance} = {
      enable = true;
      testnet = cfg.network == "testnet";
      rpc.port = 8332;
      extraConfig = ''
        txindex=1
      '';
    };

    networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [ ];

    users.users.lightning-knd = {
      isSystemUser = true;
      group = "lightning-knd";
      extraGroups = [ "cockroachdb" ];
    };
    users.groups.lightning-knd = { };

    # fix me, we need to wait for the database to start first
    systemd.services.lightning-knd = {
      wantedBy = [ "multi-user.target" ];
      after = [
        "network.target"
        "cockroachdb.service"
        "bitcoind.service"
      ];
      environment = {
        KND_LOG_LEVEL = lib.mkDefault cfg.logLevel;
        KND_PEER_PORT = lib.mkDefault (toString cfg.peerPort);
        KND_NODE_NAME = lib.mkDefault cfg.nodeAlias;
        KND_DATABASE_HOST = lib.mkDefault "localhost";
        KND_DATABASE_PORT = lib.mkDefault "26257";
        KND_DATABASE_USER = lib.mkDefault "lightning-knd";
        KND_DATABASE_NAME = lib.mkDefault "lightning_knd";
        KND_DATABASE_CA_CERT_PATH = lib.mkDefault "/var/lib/cockroachdb/certs/ca.crt";
        KND_DATABASE_CLIENT_CERT_PATH = lib.mkDefault "/var/lib/cockroachdb/certs/client.lightning-knd.crt";
        KND_DATABASE_CLIENT_KEY_PATH = lib.mkDefault "/var/lib/cockroachdb/certs/client.lightning-knd.key";
        KND_EXPORTER_ADDRESS = lib.mkDefault cfg.exporterAddress;
        KND_REST_API_ADDRESS = lib.mkDefault cfg.restApiAddress;
        KND_BITCOIN_COOKIE_PATH = lib.mkDefault "/var/lib/lightning-knd/.cookie";
        KND_CERTS_DIR = lib.mkDefault "/var/lib/lightning-knd/certs";
        KND_BITCOIN_NETWORK = lib.mkDefault cfg.network;

        KND_BITCOIN_RPC_HOST = lib.mkDefault "127.0.0.1";
        KND_BITCOIN_RPC_PORT = lib.mkDefault (toString bitcoinCfg.rpc.port);
      };
      path = [
        config.services.cockroachdb.package
        bitcoinCfg.package # for cli
        pkgs.util-linux # setpriv
      ];
      script = ''
        set -euo pipefail
        exec ${lib.getExe cfg.package}
      '';
      serviceConfig = {
        ExecStartPre = "+${pkgs.writeShellScript "setup" ''
          setpriv --reuid bitcoind-${bitcoind-instance} \
                  --regid bitcoind-${bitcoind-instance} \
                  --clear-groups \
                  --inh-caps=-all -- \
            bitcoin-cli \
              -datadir=${bitcoinCfg.dataDir} \
              -rpccookiefile=${bitcoinCfg.dataDir}/.cookie \
              -rpcconnect=127.0.0.1 \
              -rpcport=${toString bitcoinCfg.rpc.port} \
              -rpcwait getblockchaininfo
          install -m755 ${bitcoinCfg.dataDir}/.cookie /var/lib/lightning-knd/.cookie
        ''}";
        User = "lightning-knd";
        Group = "lightning-knd";
        SupplementaryGroups = [ "cockroachdb" ];
        StateDirectory = "lightning-knd";

        # New file permissions
        UMask = "0027"; # 0640 / 0750

        # Hardening measures
        # Sandboxing (sorted by occurrence in https://www.freedesktop.org/software/systemd/man/systemd.exec.html)

        ProtectSystem = "full";
        Type = "simple";
        ProtectHome = true;
        ProtectHostname = true;
        ProtectClock = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;

        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
        PrivateMounts = true;
        MemoryDenyWriteExecute = true;
        RemoveIPC = true;

        Restart = "on-failure";

        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        RestrictRealtime = true;
        RestrictSUIDSGID = true;

        LockPersonality = true;

        # Proc filesystem
        ProcSubset = "pid";
        ProtectProc = "invisible";

        RestrictNamespaces = true;

        SystemCallArchitectures = "native";
        # blacklist some syscalls
        SystemCallFilter = [ "~@cpu-emulation @debug @keyring @mount @obsolete @privileged @setuid" ];
      };
    };
  };
}
