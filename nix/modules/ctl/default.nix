{ config, lib, self, ... }:
let
  cfg = config.kuutamo.ctl;
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
    environment.systemPackages = [ cfg.package ];
    environment.etc."system-info.toml".text = lib.mkDefault ''
      git_sha = "${self.rev or "dirty"}"
      git_commit_date = "${self.lastModifiedDate}"
    '';
  };
}
