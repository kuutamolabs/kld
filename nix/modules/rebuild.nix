{ lib, config, ... }:

let
  cfg = config.kuutamo.rebuild;
in
{
  options = {
    kuutamo.rebuild.deploymentFlake = lib.mkOption {
      type = lib.types.str;
      default = "github:kuutamolabs/deployment-example";
      description = lib.mdDoc "The flake to deploy";
    };
  };

  config = {
    system.autoUpgrade = {
      enable = true;
      allowReboot = false;
      dates = "*-*-* *:00:00";
      operation = "switch";
      randomizedDelaySec = "1800";
      flake = cfg.deploymentFlake;
      flags = [ "--refresh" "--no-update-lock-file" ];
    };
  };
}
