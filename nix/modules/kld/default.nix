{ config
, lib
, pkgs
, ...
}:
let
  cfg = config.kuutamo.kld;
  bitcoind-instance = "kld-${cfg.network}";
  bitcoinCfg = config.services.bitcoind.${bitcoind-instance};
  bitcoinCookieDir =
    if cfg.network == "regtest" then
      "${bitcoinCfg.dataDir}/regtest"
    else if cfg.network == "testnet" then
      "${bitcoinCfg.dataDir}/testnet3"
    else bitcoinCfg.dataDir;

  cockroachCfg = config.kuutamo.cockroachdb;
  electrsCfg = config.kuutamo.electrs;

  kld-cli = pkgs.runCommand "kld-cli" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    makeWrapper ${cfg.package}/bin/kld-cli $out/bin/kld-cli \
      --add-flags "--target 127.0.0.1:${toString cfg.restApiPort} --cert-path /var/lib/kld/certs/ca.pem  --macaroon-path /var/lib/kld/macaroons/admin.macaroon"
  '';

  bitcoin-cli-flags = [
    "-datadir=${bitcoinCfg.dataDir}"
    "-rpccookiefile=${bitcoinCookieDir}/.cookie"
    "-rpcconnect=127.0.0.1"
    "-rpcport=${toString bitcoinCfg.rpc.port}"
  ] ++ lib.optional (cfg.network == "regtest") "-regtest"
  ++ lib.optional (cfg.network == "testnet") "-testnet";

  bitcoin-cli = pkgs.runCommand "kld-bitcoin-cli" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    makeWrapper ${bitcoinCfg.package}/bin/bitcoin-cli $out/bin/kld-bitcoin-cli \
      --add-flags "${toString bitcoin-cli-flags}"
  '';
in
{

  imports = [
    ../bitcoind-disks.nix
  ];
  options.kuutamo.kld = {
    nodeId = lib.mkOption {
      type = lib.types.str;
      default = config.networking.hostName;
      description = ''
        Node ID used in logs
      '';
    };

    caPath = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to the CA certificate used to sign the TLS certificate
      '';
    };

    certPath = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to the TLS certificate
      '';
    };

    keyPath = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to the TLS private key
      '';
    };

    cockroachdb = {
      clientCertPath = lib.mkOption {
        type = lib.types.path;
        description = ''
          Path to the client certificate
        '';
      };
      clientKeyPath = lib.mkOption {
        type = lib.types.path;
        description = ''
          Path to the client certificate
        '';
      };
    };

    package = lib.mkOption {
      type = lib.types.package;
      description = lib.mdDoc ''
        KLD package to use
      '';
    };
    logLevel = lib.mkOption {
      type = lib.types.enum [ "error" "info" "debug" "trace" ];
      default = "info";
      example = "debug";
      description = lib.mdDoc "Log level for KLD";
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
      type = lib.types.enum [ "main" "testnet" "regtest" ];
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
        Whether to open ports used by KLD
      '';
    };
    publicAddresses = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = lib.mDoc ''
        Comma-seperated list of lightning network addresses on which the node is *directly* reachable.
      '';
    };
    exporterAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:2233";
      description = lib.mDoc ''
        Address and port to bind to for exporting metrics
      '';
    };
    restApiPort = lib.mkOption {
      type = lib.types.port;
      default = 2244;
      description = lib.mDoc ''
        Port to bind to for the REST API
      '';
    };
    apiIpAccessList = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = lib.mDoc ''
        Expose REST API to specific machines
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
    environment.systemPackages = [ kld-cli bitcoin-cli ];

    kuutamo.cockroachdb.ensureDatabases = [ "kld" ];
    kuutamo.cockroachdb.ensureUsers = [{
      name = "kld";
      ensurePermissions."DATABASE kld" = "ALL";
    }];

    services.bitcoind.${bitcoind-instance} = {
      enable = true;
      testnet = cfg.network == "testnet";
      port =
        if cfg.network == "regtest" then
          18444
        else if cfg.network == "testnet" then
          18333
        else 8333;
      rpc.port = 8332;
      extraConfig = ''
        rpcthreads=16
      '';
      extraCmdlineOptions = lib.optionals (cfg.network == "regtest") [
        "-regtest"
        "-noconnect"
      ];
    };

    networking.firewall.allowedTCPPorts = [ ]
      ++ lib.optionals cfg.openFirewall [ cfg.peerPort ];
    networking.firewall.extraCommands = lib.concatMapStrings
      (ip:
        if lib.hasInfix ":" ip then ''
          ip6tables -A nixos-fw -p tcp --source ${ip} --dport ${toString cfg.restApiPort} -j nixos-fw-accept
        '' else ''
          iptables -A nixos-fw -p tcp --source ${ip} --dport ${toString cfg.restApiPort} -j nixos-fw-accept
        '')
      cfg.apiIpAccessList;

    users.users.kld = {
      isSystemUser = true;
      group = "kld";
    };
    users.groups.kld = { };

    kuutamo.disko.bitcoindDataDir = bitcoinCfg.dataDir;

    # fix me, we need to wait for the database to start first
    systemd.services.kld = {
      wantedBy = [ "multi-user.target" ];
      after = [
        "network.target"
        "cockroachdb.service"
        "cockroachdb-setup.service"
        "bitcoind.service"
        "electrs.service"
      ];
      environment = {
        KLD_LOG_LEVEL = lib.mkDefault cfg.logLevel;
        KLD_PEER_PORT = lib.mkDefault (toString cfg.peerPort);
        KLD_NODE_ALIAS = lib.mkDefault cfg.nodeAlias;
        KLD_NODE_ID = lib.mkDefault cfg.nodeId;
        KLD_DATABASE_HOST = lib.mkDefault "localhost";
        KLD_DATABASE_PORT = lib.mkDefault (toString cockroachCfg.sql.port);
        KLD_DATABASE_USER = lib.mkDefault "kld";
        KLD_DATABASE_NAME = lib.mkDefault "kld";
        KLD_DATABASE_CA_CERT_PATH = lib.mkDefault ''/var/lib/cockroachdb-certs/ca.crt'';
        KLD_DATABASE_CLIENT_CERT_PATH = lib.mkDefault "/var/lib/kld/certs/client.kld.crt";
        KLD_DATABASE_CLIENT_KEY_PATH = lib.mkDefault "/var/lib/kld/certs/client.kld.key";
        KLD_EXPORTER_ADDRESS = lib.mkDefault cfg.exporterAddress;
        KLD_REST_API_ADDRESS = if cfg.apiIpAccessList != [ ] then "[::]:${toString cfg.restApiPort}" else "127.0.0.1:${toString cfg.restApiPort}";
        KLD_BITCOIN_COOKIE_PATH = lib.mkDefault "/var/lib/kld/.cookie";
        KLD_CERTS_DIR = lib.mkDefault "/var/lib/kld/certs";
        KLD_BITCOIN_NETWORK = lib.mkDefault cfg.network;
        KLD_BITCOIN_RPC_HOST = lib.mkDefault "127.0.0.1";
        KLD_BITCOIN_RPC_PORT = lib.mkDefault (toString bitcoinCfg.rpc.port);
        KLD_ELECTRS_URL = lib.mkDefault "${electrsCfg.address}:${toString electrsCfg.port}";
      } // lib.optionalAttrs (cfg.publicAddresses != [ ]) { KLD_PUBLIC_ADDRESSES = lib.concatStringsSep "," cfg.publicAddresses; };

      path = [
        bitcoin-cli
        pkgs.util-linux # setpriv
      ];
      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        ExecStartPre = "+${pkgs.writeShellScript "setup" ''
          setpriv --reuid bitcoind-${bitcoind-instance} \
                  --regid bitcoind-${bitcoind-instance} \
                  --clear-groups \
                  --inh-caps=-all -- \
            kld-bitcoin-cli -rpcwait getblockchaininfo
          install -m400 -o kld ${bitcoinCookieDir}/.cookie /var/lib/kld/.cookie

          install -D -m400 -o kld ${cfg.certPath} /var/lib/kld/certs/kld.crt
          install -D -m400 -o kld ${cfg.keyPath} /var/lib/kld/certs/kld.key
          install -D -m400 -o kld ${cfg.caPath} /var/lib/kld/certs/ca.pem
          install -D -m400 -o kld ${cfg.cockroachdb.clientCertPath} /var/lib/kld/certs/client.kld.crt
          install -D -m400 -o kld ${cfg.cockroachdb.clientKeyPath} /var/lib/kld/certs/client.kld.key
        ''}";
        User = "kld";
        Group = "kld";
        SupplementaryGroups = [ "cockroachdb" ];
        StateDirectory = "kld";

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
