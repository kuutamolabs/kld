{ config
, lib
, pkgs
, ...
}:
let
  cfg = config.services.cockroachdb;
in
{
  options = {
    services.cockroachdb.ensureDatabases = lib.mkOption {
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

    services.cockroachdb.ensureUsers = lib.mkOption {
      type = lib.types.listOf (lib.types.submodule {
        options = {
          name = lib.mkOption {
            type = lib.types.str;
            description = lib.mdDoc ''
              Name of the user to ensure.
            '';
          };
          passwordFile = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
            description = lib.mdDoc ''
              Path to a file containing the password for the user.
              The file should contain only the password, not any trailing newlines.
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
  };

  config = {
    services.cockroachdb.extraArgs = [
      # TODO: support TLS client certs in lightning-knd
      "--accept-sql-without-tls"
      "--socket-dir=/run/cockroachdb"
      # disable file-based logging
      "--log-dir="
    ];
    services.cockroachdb.enable = true;
    services.cockroachdb.certsDir = "/var/lib/cockroachdb/certs";
    services.cockroachdb.openPorts = true;
    services.cockroachdb.listen.address = "[::]";

    systemd.services.cockroachdb =
      let
        connectFlags = ''--certs-dir /var/lib/cockroachdb/certs --host "$hostname:26257"'';
        csql = execute: ''cockroach sql ${connectFlags} ${lib.cli.toGNUCommandLineShell {} { inherit  execute; }}'';
        sql =
          (builtins.map (database: ''CREATE DATABASE IF NOT EXISTS "${database}"'') cfg.ensureDatabases)
          ++ (builtins.map (user: ''CREATE USER IF NOT EXISTS "${user.name}"'') cfg.ensureUsers)
          ++ (lib.flatten
            (builtins.map
              (user: lib.mapAttrsToList (database: permission: ''GRANT ${permission} ON ${database} TO "${user.name}" '') user.ensurePermissions)
              cfg.ensureUsers));
      in
      {
        serviceConfig = {
          RuntimeDirectory = "cockroachdb";
          WorkingDirectory = "/var/lib/cockroachdb";
          # for cli
          path = [ cfg.package ];
          # we need to run this as root since do not have a password yet.
          ExecStartPost = "+${pkgs.writeShellScript "setup" ''
          set -eu -o pipefail
          export PATH=$PATH:${cfg.package}/bin

          # check if this is the primary database node
          if [[ -f /var/lib/cockroachdb/certs/client.root.crt ]]; then
            hostname=$(${lib.getExe pkgs.openssl} x509 -text -noout -in /var/lib/cockroachdb/certs/node.crt | grep -oP '(?<=DNS:).*')

            if [[ ! -f /var/lib/cockroachdb/.cluster-init ]]; then
              cockroach init ${connectFlags}
              touch /var/lib/cockroachdb/.cluster-init
            fi
            ${csql sql}
            # FIXME: this might log the password in the journal -> just use a cert here.
            ${lib.concatMapStringsSep "\n" (user: lib.optionalString (user.passwordFile != null) ''
              cockroach ${connectFlags} sql --execute "ALTER USER \"${user.name}\" WITH PASSWORD '$(cat ${user.passwordFile})'"
            '') cfg.ensureUsers}
          fi
        ''}";
        };
      };
  };
}
