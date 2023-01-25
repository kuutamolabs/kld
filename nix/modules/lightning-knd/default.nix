{ config
, lib
, pkgs
, ...
}:
let
  cfg = config.kuutamo.lightning-knd;
  bitcoind-instance = "lightning-knd-${cfg.network}";
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

    network = lib.mkOption {
      # Our bitcoind module does not handle anything but bitcoind and testnet at the moment.
      # We might however not need more than that.
      #type = lib.types.enum [ "bitcoin" "testnet" "signet" "regtest" ];
      type = lib.types.enum [ "bitcoin" "testnet" ];
      default = "bitcoin";
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
  };

  imports = [
    ../cockroachdb.nix
  ];

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
      after = [ "network.target" "cockroachdb.service" ];
      environment = {
        KND_DATABASE_HOST = lib.mkDefault "/run/cockroachdb";
        KND_DATABASE_PORT = lib.mkDefault "26257";
        KND_DATABASE_USER = lib.mkDefault "lightning-knd";
        KND_DATABASE_NAME = lib.mkDefault "lightning_knd";
        KND_EXPORTER_ADDRESS = lib.mkDefault "127.0.0.1:2233";
        KND_REST_API_ADDRESS = lib.mkDefault "127.0.0.1:2244";
        KND_BITCOIN_COOKIE_PATH = lib.mkDefault "${config.services.bitcoind.${bitcoind-instance}.dataDir}/.cookie";
        KND_BITCOIN_NETWORK = lib.mkDefault cfg.network;
      };
      path = [
        config.services.cockroachdb.package
      ];
      serviceConfig = {
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

        ExecStartPre = "+${pkgs.acl}/bin/setfacl -m u:lightning-knd:rw /run/cockroachdb/.s.PGSQL.26257";
        ExecStart = lib.getExe cfg.package;
      };
    };
  };
}
