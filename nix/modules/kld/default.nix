{ config
, lib
, pkgs
, self
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

  kld-cli = pkgs.runCommand "kld-cli" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    makeWrapper ${cfg.package}/bin/kld-cli $out/bin/kld-cli \
      --add-flags "--target ${cfg.restApiAddress} --cert-path /var/lib/kld/certs/ca.pem  --macaroon-path /var/lib/kld/macaroons/admin.macaroon"
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
      type = lib.types.enum [ "info" "debug" "trace" ];
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
        Comma-seperated list of ip addresses on which the lightning node is *directly* reachable.
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
    environment.systemPackages = [ kld-cli bitcoin-cli ];
    environment.etc."system-info.toml".text = lib.mkDefault ''
      git_sha = "${self.rev or "dirty"}"
      git_commit_date = "${self.lastModifiedDate}"
    '';
    system.activationScripts.kld-node-upgrade = ''
      ${config.systemd.package}/bin/systemd-run --collect --unit nixos-upgrade echo level=info message=\"kld node updated\" $(kld-ctl system-info --inline)
    '';

    kuutamo.cockroachdb.ensureDatabases = [ "kld" ];
    kuutamo.cockroachdb.ensureUsers = [{
      name = "kld";
      ensurePermissions."DATABASE kld" = "ALL";
    }];

    services.bitcoind.${bitcoind-instance} = {
      enable = true;
      testnet = cfg.network == "testnet";
      rpc.port = 8332;
      extraConfig = ''
        txindex=1
      '';
      extraCmdlineOptions = lib.optionals (cfg.network == "regtest") [
        "-regtest"
        "-noconnect"
      ];
    };

    networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [ ];

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
        "bitcoind.service"
      ];
      environment = {
        KLD_LOG_LEVEL = lib.mkDefault cfg.logLevel;
        KLD_PEER_PORT = lib.mkDefault (toString cfg.peerPort);
        KLD_NODE_NAME = lib.mkDefault cfg.nodeAlias;
        KLD_DATABASE_HOST = lib.mkDefault "localhost";
        KLD_DATABASE_PORT = lib.mkDefault (toString cockroachCfg.sql.port);
        KLD_DATABASE_USER = lib.mkDefault "kld";
        KLD_DATABASE_NAME = lib.mkDefault "kld";
        KLD_DATABASE_CA_CERT_PATH = lib.mkDefault ''/var/lib/cockroachdb-certs/ca.crt'';
        KLD_DATABASE_CLIENT_CERT_PATH = lib.mkDefault "/var/lib/kld/certs/client.kld.crt";
        KLD_DATABASE_CLIENT_KEY_PATH = lib.mkDefault "/var/lib/kld/certs/client.kld.key";
        KLD_PUBLIC_ADDRESSES = lib.concatStringsSep "," cfg.publicAddresses;
        KLD_EXPORTER_ADDRESS = lib.mkDefault cfg.exporterAddress;
        KLD_REST_API_ADDRESS = lib.mkDefault cfg.restApiAddress;
        KLD_BITCOIN_COOKIE_PATH = lib.mkDefault "/var/lib/kld/.cookie";
        KLD_CERTS_DIR = lib.mkDefault "/var/lib/kld/certs";
        KLD_BITCOIN_NETWORK = lib.mkDefault cfg.network;
        KLD_BITCOIN_RPC_HOST = lib.mkDefault "127.0.0.1";
        KLD_BITCOIN_RPC_PORT = lib.mkDefault (toString bitcoinCfg.rpc.port);
      };
      path = [
        bitcoin-cli
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
