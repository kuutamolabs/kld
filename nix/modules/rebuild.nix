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
    kuutamo.rebuild.accessTokens = lib.mkOption {
      type = lib.types.str;
      default = "github.com=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
      description = lib.mdDoc "The token to access deployment flake";
    };
  };

  config = {
    system.autoUpgrade = {
      enable = true;
      allowReboot = true;
      # XXX
      # let the kld-autograde service of other nodes to trigger this,
      # and should not self trigger,
      # so the date is set on the day David and Lucy meet on the moon
      dates = "2077-03-16 00:00:00";
      operation = "switch";
      randomizedDelaySec = "1800";
      flake = cfg.deploymentFlake;
      flags = [
        "--refresh"
        "--no-update-lock-file"
        "--option"
        "--accept-flake-config"
        "true"
      ];
    };
    nix.settings.access-tokens = cfg.accessTokens;
  };
}
