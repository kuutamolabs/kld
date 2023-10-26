{ pkgs, config, lib, ... }:
let
  cfg = config.kuutamo.network;
in
{
  options = {
    kuutamo.network.interface = lib.mkOption {
      type = lib.types.str;
      default = "eth0";
      description = "Will be ignored if also a mac address is provided.";
    };

    kuutamo.network.macAddress = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Used to identify the public interface.";
    };

    kuutamo.network.ipv4.address = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
    };

    kuutamo.network.ipv4.cidr = lib.mkOption {
      type = lib.types.int;
      default = 32;
    };

    kuutamo.network.ipv4.gateway = lib.mkOption {
      type = lib.types.str;
    };

    kuutamo.network.ipv6.address = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
    };

    kuutamo.network.ipv6.cidr = lib.mkOption {
      type = lib.types.int;
      default = 128;
    };

    kuutamo.network.ipv6.gateway = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
    };
  };

  config = {

    assertions = [{
      assertion = cfg.ipv4.address != null || cfg.ipv6.address != null;
      message = ''
        At least one ipv4 or ipv6 address must be configured
      '';
    }
      {
        assertion = cfg.ipv4.address != null -> cfg.ipv4.gateway != null;
        message = ''
          No ipv4 gateway configured
        '';
      }
      {
        assertion = cfg.ipv6.address != null -> cfg.ipv6.gateway != null;
        message = ''
          No ipv6 gateway configured
        '';
      }];

    # we just have one interface called 'eth0'
    networking.usePredictableInterfaceNames = false;

    systemd.services.log-network-status = {
      wantedBy = [ "multi-user.target" ];
      # No point in restarting this. We just need this after boot
      restartIfChanged = false;

      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        ExecStart = [
          # if we cannot get online still print what interfaces we have
          "-${pkgs.systemd}/lib/systemd/systemd-networkd-wait-online -i eth0"
          "${pkgs.iproute2}/bin/ip -c addr"
          "${pkgs.iproute2}/bin/ip -c -6 route"
          "${pkgs.iproute2}/bin/ip -c -4 route"
        ];
      };
    };

    systemd.network = {
      enable = true;
      networks."ethernet".extraConfig =
        if (cfg.ipv4.cidr == 32 || cfg.ipv6.cidr == 128) then
          ''
            [Match]
            ${if cfg.macAddress == null then ''
              Name = ${cfg.interface}
            '' else  ''
              MACAddress = ${cfg.macAddress}
            ''}

            [Address]
            ${lib.optionalString (cfg.ipv4.address != null) ''
              Address = ${cfg.ipv4.address}
              Peer = ${cfg.ipv4.gateway}
            ''}
            ${lib.optionalString (cfg.ipv6.address != null) ''
              Address = ${cfg.ipv6.address}
              Peer = ${cfg.ipv6.gateway}
            ''}

            [Network]
            ${lib.optionalString (cfg.ipv4.address != null) ''
              Gateway = ${cfg.ipv4.gateway}
            ''}
            ${lib.optionalString (cfg.ipv6.address != null) ''
              Gateway = ${cfg.ipv6.gateway}
            ''}
          ''
        else
          ''
            [Match]
            ${if cfg.macAddress == null then ''
              Name = ${cfg.interface}
            '' else  ''
              MACAddress = ${cfg.macAddress}
            ''}

            [Network]
            ${lib.optionalString (cfg.ipv4.address != null) ''
              Address = ${cfg.ipv4.address}/${toString cfg.ipv4.cidr}
              Gateway = ${cfg.ipv4.gateway}
            ''}
            ${lib.optionalString (cfg.ipv6.address != null) ''
              Address = ${cfg.ipv6.address}/${toString cfg.ipv4.cidr}
              Gateway = ${cfg.ipv6.gateway}
            ''}
          '';
    };
  };
}
