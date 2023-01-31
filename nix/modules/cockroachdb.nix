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
      "--socket-dir=/run/cockroachdb"
    ];
    services.cockroachdb.enable = true;
    # TODO: setup clustering and ssl certificates.
    services.cockroachdb.insecure = true;

    systemd.services.cockroachdb =
      let
        csql = execute:
          "cockroach sql ${lib.cli.toGNUCommandLineShell {} {
              url = "postgres://?host=/run/cockroachdb/&port=26257&sslmode=disable";
              inherit execute;
          }}";
        sql = (builtins.map (database: ''CREATE DATABASE IF NOT EXISTS "${database}"'') cfg.ensureDatabases)
          ++ (builtins.map (user: ''CREATE USER IF NOT EXISTS "${user.name}"'') cfg.ensureUsers)
          ++ (lib.flatten
          (builtins.map
            (user: lib.mapAttrsToList (database: permission: ''GRANT ${permission} ON ${database} TO "${user.name}"'') user.ensurePermissions)
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
          #set -eux -o pipefail
          export PATH=$PATH:${cfg.package}/bin
          while ! ${csql ""} 2>/dev/null; do
            if ! kill -0 "$MAINPID"; then exit 1; fi
            sleep 0.1
          done
          ${csql sql}
        ''}";
        };
      };
  };
}
