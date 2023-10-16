{ lib, config, pkgs, ... }:
{
  options = {
    kuutamo.upgrade.deploymentFlake = lib.mkOption {
      type = lib.types.str;
      default = "github:kuutamolabs/deployment-example";
      description = lib.mdDoc "The flake to deploy";
    };
    kuutamo.upgrade.time = lib.mkOption {
      type = lib.types.str;
      default = "*-*-* 0:00:00";
      description = "the order for upgrade in the cluster";
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
          nixos-rebuild = "${config.system.build.nixos-rebuild}/bin/nixos-rebuild";
          nix-collect-garbage = "${config.nix.package.out}/bin/nix-collect-garbage";
          kexec = "${pkgs.kexec-tools}/bin/kexec";
          systemctl = "${config.systemd.package}/bin/systemctl";
          cpio = "${pkgs.cpio}/bin/cpio";
          gzip = "${pkgs.gzip}/bin/gzip";
        in
        ''
          ${nixos-rebuild} switch \
            --no-update-lock-file \
            --option accept-flake-config true \
            --option access-tokens $ACCESS_TOKENS \
            --flake ${config.kuutamo.upgrade.deploymentFlake}
          ${nix-collect-garbage}

          # Unload kexec if existing
          # then put disk encrypted key into the new initrd
          ${kexec} -u
          p=$(${readlink} -f /nix/var/nix/profiles/system)
          initrd=$(mktemp -d)
          mkdir -p $initrd/initrd
          cp $p/initrd $initrd/current-initrd
          cp /var/lib/secrets/disk_encryption_key $initrd/initrd/key-file
          cd $initrd/initrd
          find . |${cpio} -H newc -o | ${gzip} -9 >> ../current-initrd
          ${kexec} --load $p/kernel --initrd=$initrd/current-initrd --reuse-cmdline && ${systemctl} kexec
        '';

      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
    };
    systemd.extraConfig = ''
      DefaultTimeoutStopSec=900s
    '';

    systemd.timers.nixos-upgrade = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = config.kuutamo.upgrade.time;
      };
    };
  };
}
