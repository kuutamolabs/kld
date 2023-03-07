{ config, lib, pkgs, utils, ... }:

let
  cfg = config.kuutamo.cockroachdb;
  crdb = cfg.package;

  cockroach-cli = pkgs.runCommand "cockroach-wrapper" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    makeWrapper ${cfg.package}/bin/cockroach $out/bin/cockroach \
      --add-flags " --certs-dir=${cfg.certsDir} --host=${config.networking.fqdnOrHostName} "
  '';

  logConfig = {
    sinks.file-groups = { };
    sinks.stderr = {
      channels = "all";
      filter = "NONE";
      redact = true;
      redactable = true;
      exit-on-error = true;
    };
  };

  csql = execute: ''cockroach sql ${lib.cli.toGNUCommandLineShell {} { inherit  execute; }}'';
  initialSql =
    (builtins.map (database: ''CREATE DATABASE IF NOT EXISTS "${database}"'') cfg.ensureDatabases)
    ++ (builtins.map (user: ''CREATE USER IF NOT EXISTS "${user.name}"'') cfg.ensureUsers)
    ++ (lib.flatten
      (builtins.map
        (user: lib.mapAttrsToList (database: permission: ''GRANT ${permission} ON ${database} TO "${user.name}" '') user.ensurePermissions)
        cfg.ensureUsers));

  startupCommand = utils.escapeSystemdExecArgs
    ([
      # Basic startup
      "${crdb}/bin/cockroach"
      "start"
      "--store=/var/lib/cockroachdb"
      "--socket-dir=/run/cockroachdb"
      # disable file-based logging
      "--log-config-file=${pkgs.writeText "cockroach-log-config.yaml" (builtins.toJSON logConfig)}"

      # WebUI settings
      "--http-addr=${cfg.http.address}:${toString cfg.http.port}"

      # Cluster listen address
      "--listen-addr=${cfg.listen.address}:${toString cfg.listen.port}"

      # Cache and memory settings.
      "--cache=${cfg.cache}"
      "--max-sql-memory=${cfg.maxSqlMemory}"

      # Certificate/security settings.
      (if cfg.insecure then "--insecure" else "--certs-dir=${cfg.certsDir}")
    ]
    ++ lib.optional (cfg.join != null) "--join=${cfg.join}"
    ++ lib.optional (cfg.locality != null) "--locality=${cfg.locality}"
    ++ cfg.extraArgs);

  addressOption = descr: defaultPort: {
    address = lib.mkOption {
      type = lib.types.str;
      default = "[::]";
      description = lib.mdDoc "Address to bind to for ${descr}";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = defaultPort;
      description = lib.mdDoc "Port to bind to for ${descr}";
    };
  };
in
{
  options = {
    kuutamo.cockroachdb = {
      listen = {
        address = lib.mkOption {
          type = lib.types.str;
          default = "[::]";
          description = lib.mdDoc "Address to bind to for listen";
        };
        port = lib.mkOption {
          type = lib.types.port;
          default = 26257;
          description = lib.mdDoc "Port to bind to for listen";
        };
      };

      http = addressOption "http-based Admin UI" 8080;

      locality = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = lib.mdDoc ''
          An ordered, comma-separated list of key-value pairs that describe the
          topography of the machine. Topography might include country,
          datacenter or rack designations. Data is automatically replicated to
          maximize diversities of each tier. The order of tiers is used to
          determine the priority of the diversity, so the more inclusive
          localities like country should come before less inclusive localities
          like datacenter.  The tiers and order must be the same on all nodes.
          Including more tiers is better than including fewer. For example:

          ```
              country=us,region=us-west,datacenter=us-west-1b,rack=12
              country=ca,region=ca-east,datacenter=ca-east-2,rack=4

              planet=earth,province=manitoba,colo=secondary,power=3
          ```
        '';
      };

      join = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = lib.mdDoc "The addresses for connecting the node to a cluster.";
      };

      insecure = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = lib.mdDoc "Run in insecure mode.";
      };

      certsDir = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = "/var/lib/cockroachdb/certs";
        description = lib.mdDoc "The path to the certificate directory.";
      };

      user = lib.mkOption {
        type = lib.types.str;
        default = "cockroachdb";
        description = lib.mdDoc "User account under which CockroachDB runs";
      };

      group = lib.mkOption {
        type = lib.types.str;
        default = "cockroachdb";
        description = lib.mdDoc "User account under which CockroachDB runs";
      };

      openPorts = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = lib.mdDoc "Open firewall ports for cluster communication by default";
      };

      cache = lib.mkOption {
        type = lib.types.str;
        default = "25%";
        description = lib.mdDoc ''
          The total size for caches.

          This can be a percentage, expressed with a fraction sign or as a
          decimal-point number, or any bytes-based unit. For example,
          `"25%"`, `"0.25"` both represent
          25% of the available system memory. The values
          `"1000000000"` and `"1GB"` both
          represent 1 gigabyte of memory.

        '';
      };

      maxSqlMemory = lib.mkOption {
        type = lib.types.str;
        default = "25%";
        description = lib.mdDoc ''
          The maximum in-memory storage capacity available to store temporary
          data for SQL queries.

          This can be a percentage, expressed with a fraction sign or as a
          decimal-point number, or any bytes-based unit. For example,
          `"25%"`, `"0.25"` both represent
          25% of the available system memory. The values
          `"1000000000"` and `"1GB"` both
          represent 1 gigabyte of memory.
        '';
      };

      package = lib.mkOption {
        type = lib.types.package;
        description = lib.mdDoc ''
          The CockroachDB derivation to use for running the service.

          This would primarily be useful to enable Enterprise Edition features
          in your own custom CockroachDB build (Nixpkgs CockroachDB binaries
          only contain open source features and open source code).
        '';
      };

      ensureDatabases = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = lib.mdDoc ''
          Ensures that the specified databases exist.
          This option will never delete existing databases, especially not when the value of this
          option is changed. This means that databases created once through this option or
          otherwise have to be removed manually.
        '';
        example = [
          "gitea"
          "nextcloud"
        ];
      };

      ensureUsers = lib.mkOption {
        type = lib.types.listOf (lib.types.submodule {
          options = {
            name = lib.mkOption {
              type = lib.types.str;
              description = lib.mdDoc ''
                Name of the user to ensure.
              '';
            };

            ensurePermissions = lib.mkOption {
              type = lib.types.attrsOf lib.types.str;
              default = { };
              description = lib.mdDoc ''
                Permissions to ensure for the user, specified as an attribute set.
                The attribute names specify the database and tables to grant the permissions for.
                The attribute values specify the permissions to grant. You may specify one or
                multiple comma-separated SQL privileges here.

                For more information on how to specify the target
                and on which privileges exist, see the
                [GRANT syntax](https://www.cockroachlabs.com/docs/v22.2/grant).
                The attributes are used as `GRANT ''${attrValue} ON ''${attrName}`.
              '';
              example = lib.literalExpression ''
                {
                  "DATABASE \"nextcloud\"" = "ALL PRIVILEGES";
                  "ALL TABLES IN SCHEMA public" = "ALL PRIVILEGES";
                }
              '';
            };
          };
        });
        default = [ ];
        description = lib.mdDoc ''
          Ensures that the specified users exist and have at least the ensured permissions.
          The users will be identified using peer authentication. This authenticates the Unix user with the
          same name only, and that without the need for a password.
          This option will never delete existing users or remove permissions, especially not when the value of this
          option is changed. This means that users created and permissions assigned once through this option or
          otherwise have to be removed manually.
        '';
        example = lib.literalExpression ''
          [
            {
              name = "nextcloud";
              ensurePermissions = {
                "DATABASE nextcloud" = "ALL PRIVILEGES";
              };
            }
            {
              name = "superuser";
              ensurePermissions = {
                "ALL TABLES IN SCHEMA public" = "ALL PRIVILEGES";
              };
            }
          ]
        '';
      };

      extraArgs = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "--advertise-addr" "[fe80::f6f2:::]" ];
        description = lib.mdDoc ''
          Extra CLI arguments passed to {command}`cockroach start`.
          For the full list of supported argumemnts, check <https://www.cockroachlabs.com/docs/stable/cockroach-start.html#flags>
        '';
      };
    };
  };

  config = {
    assertions = [
      {
        assertion = !cfg.insecure -> cfg.certsDir != null;
        message = "CockroachDB must have a set of SSL certificates (.certsDir), or run in Insecure Mode (.insecure = true)";
      }
    ];

    environment.systemPackages = [ cockroach-cli ];

    users.users = lib.optionalAttrs (cfg.user == "cockroachdb") {
      cockroachdb = {
        description = "CockroachDB Server User";
        uid = config.ids.uids.cockroachdb;
        inherit (cfg) group;
      };
    };

    users.groups = lib.optionalAttrs (cfg.group == "cockroachdb") {
      cockroachdb.gid = config.ids.gids.cockroachdb;
    };

    networking.firewall.allowedTCPPorts = lib.optionals cfg.openPorts
      [ cfg.http.port cfg.listen.port ];

    systemd.services.cockroachdb =
      {
        description = "CockroachDB Server";
        documentation = [ "man:cockroach(1)" "https://www.cockroachlabs.com" ];

        after = [ "network.target" "time-sync.target" ];
        requires = [ "time-sync.target" ];
        wantedBy = [ "multi-user.target" ];

        unitConfig.RequiresMountsFor = "/var/lib/cockroachdb";

        serviceConfig =
          {
            ExecStart = startupCommand;
            Type = "notify";
            User = cfg.user;
            StateDirectory = "cockroachdb";
            StateDirectoryMode = "0700";
            RuntimeDirectory = "cockroachdb";
            WorkingDirectory = "/var/lib/cockroachdb";

            # for cli
            path = [ cockroach-cli ];

            Restart = "always";

            # we need to run this as root since do not have a password yet.
            ExecStartPost = "+${pkgs.writeShellScript "setup" ''
              set -eu -o pipefail
              export PATH=$PATH:${cfg.package}/bin

              # check if this is the primary database node
              if [[ -f /var/lib/cockroachdb-certs/client.root.crt ]]; then

                if [[ ! -f /var/lib/cockroachdb/.cluster-init ]]; then
                  cockroach-rpc init
                  touch /var/lib/cockroachdb/.cluster-init
                fi
                ${pkgs.iproute2}/bin/ss -tlpn
                ${csql initialSql}
              fi
            ''}";

            # A conservative-ish timeout is alright here, because for Type=notify
            # cockroach will send systemd pings during startup to keep it alive
            TimeoutStopSec = 60;
            RestartSec = 10;
          };
      };
  };
}
