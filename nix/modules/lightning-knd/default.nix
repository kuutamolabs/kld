{ config
, lib
, ...
}:
let
  cfg = config.kuutamo.lightning-knd;
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

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to open ports used by lightning-knd
      '';
    };
    publicAddresses = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        Comma-seperated list of ip addresses on which the lightning is *directly* reachable.
      '';
    };
  };

  imports = [
  ];

  config = {
    # for cli
    environment.systemPackages = [ cfg.package ];

    networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [ ];

    # fix me, we need to wait for the database to start first
    systemd.services.lightning-knd = {
      serviceConfig = {
        DynamicUser = true;
        User = "lightning-knd";
        Group = "lightning-knd";
        ExecStart = "${lib.getExe cfg.package}";
      };
    };
  };
}
