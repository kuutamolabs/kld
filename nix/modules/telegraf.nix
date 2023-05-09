{ config, pkgs, lib, ... }:
{
  options = {
    kuutamo.telegraf.configHash = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "telegraf config hash";
    };

    kuutamo.telegraf.hasMonitoring = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "has monitoring setting or not";
    };
  };
  config = {
    systemd.services.telegraf.path = [ pkgs.nvme-cli ];

    services.telegraf = {
      enable = true;
      environmentFiles = [
        /var/lib/secrets/telegraf
        (pkgs.writeTextFile {
          name = "monitoring-passwordhash";
          text = config.kuutamo.telegraf.configHash;
        })
      ];
      extraConfig = {
        agent.interval = "60s";
        inputs = {
          prometheus.urls = [
            "http://localhost:3030/metrics"
            "http://localhost:2233/metrics"
          ];
          prometheus.metric_version = 2;
          kernel_vmstat = { };
          smart = {
            path = pkgs.writeShellScript "smartctl" ''
              exec /run/wrappers/bin/sudo ${pkgs.smartmontools}/bin/smartctl "$@"
            '';
          };
          mdstat = { };
          system = { };
          mem = { };
          file =
            [
              {
                data_format = "influx";
                file_tag = "name";
                files = [ "/var/log/telegraf/*" ];
              }
            ]
            ++ lib.optional (lib.any (fs: fs == "ext4") config.boot.supportedFilesystems) {
              name_override = "ext4_errors";
              files = [ "/sys/fs/ext4/*/errors_count" ];
              data_format = "value";
            };
          exec = [
            {
              ## Commands array
              commands = [
                (pkgs.writeShellScript "ipv6-dad-check" ''
                  ${pkgs.iproute2}/bin/ip --json addr | \
                    ${pkgs.jq}/bin/jq -r 'map(.addr_info) | flatten(1) | map(select(.dadfailed == true)) | map(.local) | @text "ipv6_dad_failures count=\(length)i"'
                '')
              ];
              data_format = "influx";
            }
          ];
          systemd_units = { };
          swap = { };
          disk.tagdrop = {
            fstype = [ "tmpfs" "ramfs" "devtmpfs" "devfs" "iso9660" "overlay" "aufs" "squashfs" ];
            device = [ "rpc_pipefs" "lxcfs" "nsfs" "borgfs" ];
          };
          diskio = { };
        };
        outputs =
          let
            kmonitor =
              if config.kuutamo.telegraf.hasMonitoring then {
                http = {
                  url = "$MONITORING_URL";
                  data_format = "prometheusremotewrite";
                  username = "$MONITORING_USERNAME";
                  password = "$MONITORING_PASSWORD";
                };
              } else { };
          in
          {
            prometheus_client = {
              listen = ":9273";
              metric_version = 2;
            };
          } // kmonitor;
      };
    };
    security.sudo.extraRules = [
      {
        users = [ "telegraf" ];
        commands = [
          {
            command = "${pkgs.smartmontools}/bin/smartctl";
            options = [ "NOPASSWD" ];
          }
        ];
      }
    ];
    # avoid logging sudo use
    security.sudo.configFile = ''
      Defaults:telegraf !syslog,!pam_session
    '';
    # create dummy file to avoid telegraf errors
    systemd.tmpfiles.rules = [
      "f /var/log/telegraf/dummy 0444 root root - -"
    ];
  };
}

