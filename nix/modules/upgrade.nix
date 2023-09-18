{ lib, config, pkgs, ... }:
{
  options = {
    kuutamo.upgrade.deploymentFlake = lib.mkOption {
      type = lib.types.str;
      default = "github:kuutamolabs/deployment-example";
      description = lib.mdDoc "The flake to deploy";
    };
    kuutamo.upgrade.tokenHash = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "access-tokens hash";
    };
  };

  config = {
    systemd.services.nixos-upgrade = {
      description = "Kuutamo customized NixOS Upgrade";

      restartIfChanged = false;
      unitConfig.X-StopOnRemoval = false;

      serviceConfig.Type = "oneshot";
      serviceConfig.EnvironmentFile = [
        /var/lib/secrets/access-tokens
      ];

      environment = config.nix.envVars // {
        inherit (config.environment.sessionVariables) NIX_PATH;
        HOME = "/root";
        TOKEN_HAHS = config.kuutamo.upgrade.tokenHash;
      } // config.networking.proxy.envVars;

      path = with pkgs; [
        coreutils
        gnutar
        xz.bin
        gzip
        gitMinimal
        config.nix.package.out
        config.programs.ssh.package
      ];

      script =
        let
          nixos-rebuild = "${config.system.build.nixos-rebuild}/bin/nixos-rebuild";
          shutdown = "${config.systemd.package}/bin/shutdown";
        in
        ''
          ${nixos-rebuild} switch \
            --no-update-lock-file \
            --option --accept-flake-config true \
            --option --access-tokens $ACCESS_TOKENS \
            --flake ${config.kuutamo.upgrade.deploymentFlake}
          ${shutdown} -r
        '';

      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
    };
    systemd.extraConfig = ''
      DefaultTimeoutStopSec=900s
    '';
  };
}
