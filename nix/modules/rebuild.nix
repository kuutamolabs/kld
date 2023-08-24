{ lib, config, ... }:

let
  cfg = config.kuutamo.rebuild;
in
{
  options = {
    kuutamo.rebuild.deployment_flake = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = lib.mdDoc "The flake to deploy";
    };
  };

  config = {
    assertions = [{
      assertion = cfg.deployment_flake != null;
      message = ''
        Deployment flake must be configured
      '';
    }];

    system.autoUpgrade = {
      enable = true;
      allowReboot = false;
      dates = "*-*-* *:00:00";
      operation = "switch";
      randomizedDelaySec = "1800";
      flake = cfg.deployment_flake;
      flags = [ "--refresh" "--no-update-lock-file" ];
    };
  };
}
