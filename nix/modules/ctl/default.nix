{ config, lib, self, ... }:
let
  cfg = config.kuutamo.ctl;
  systemd = config.systemd.package;
in
{
  options.kuutamo.ctl = {
    package = lib.mkOption {
      type = lib.types.package;
      description = lib.mdDoc ''
        kld-ctl package to use
      '';
    };
  };

  config = {
    system.activationScripts.node-upgrade = ''
      ${systemd}/bin/systemd-run --collect --unit system-upgrade echo level=info message=\"kld node updated\" $(${cfg.package}/bin/kld-ctl system-info --inline)
    '';
    environment.systemPackages = [ cfg.package ];
    environment.etc."system-info.toml".text = lib.mkDefault ''
      git_sha = "${self.rev or "dirty"}"
      git_commit_date = "${self.lastModifiedDate}"
      git_commit_date = "${self.lastModifiedDate}"
      deployment_flake = "${config.kuutamo.upgrade.deploymentFlake}"
    '';
  };
}
