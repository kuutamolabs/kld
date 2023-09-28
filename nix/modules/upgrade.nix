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
    systemd.services.prepare-kexec.enable = false;

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
          readlink = "${pkgs.coreutils}/bin/readlink";
          cat = "${pkgs.coreutils}/bin/cat";
          nixos-rebuild = "${config.system.build.nixos-rebuild}/bin/nixos-rebuild";
          nix-collect-garbage = "${config.nix.package.out}/bin/nix-collect-garbage";
          kexec = "${pkgs.kexec-tools}/bin/kexec";
          systemctl = "${config.systemd.package}/bin/systemctl";
        in
        ''
          ${nixos-rebuild} switch \
            --no-update-lock-file \
            --option --accept-flake-config true \
            --option --access-tokens $ACCESS_TOKENS \
            --flake ${config.kuutamo.upgrade.deploymentFlake}
          ${nix-collect-garbage}
          p=$(${readlink} -f /nix/var/nix/profiles/system)
          if cat /proc/cmdline | grep 'disk-key'; then
            ${kexec} --load $p/kernel --initrd=$p/initrd \
              --reuse-cmdline \
              init=$p/init && ${systemctl} kexec
          else
            ${kexec} --load $p/kernel --initrd=$p/initrd \
              --append="loglevel=4 net.ifnames=0 disk-key=$(${cat} /var/lib/secrets/disk_encryption_key)" \
              init=$p/init && ${systemctl} kexec
          fi
        '';

      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
    };
    systemd.extraConfig = ''
      DefaultTimeoutStopSec=900s
    '';
  };
}
