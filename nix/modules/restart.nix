{ lib, config, ... }:

let
  cfg = config.kuutamo.restart;
in
{
  options = {
    kuutamo.restart.target = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = lib.mdDoc "The restart node in the cluster";
    };
    kuutamo.restart.order = lib.mkOption {
      type = lib.types.int;
      default = 0;
      description = lib.mdDoc "The order to exec the restart in the cluster";
    };
  };

  config = lib.mkIf (cfg.target != null) {
    systemd.services.selfReboot = {
      description = "regularly reboot " + lib.optionalString (cfg.target != null) "${cfg.target}";
      wantedBy = [ "network.target" ];
    };
  };
}
