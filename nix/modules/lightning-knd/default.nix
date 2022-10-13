{ config
, lib
, pkgs
, ...
}:
let
  lightning-knd = pkgs.callPackage ../../pkgs/lightning-knd.nix { };
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
    environment.systemPackages = [
    ];

    environment.variables = {
    };

    networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [
    ];
  };
}
