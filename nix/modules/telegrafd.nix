{ pkgs, lib, config, ... }: {
  systemd.services.telegraf.path = [ pkgs.nvme-cli ];

  services.telegraf = {
    enable = true;
    extraConfig = {
      agent.interval = "60s";
      inputs = {
        prometheus.urls = [
          "http://localhost:3030/metrics"
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
          monitor = if config.kuutamo.telegraf.url == "" then { } else {
            influxdb = {
              urls = [ config.kuutamo.telegraf.url or "" ];
              username = config.kuutamo.telegraf.username or "";
              password = config.kuutamo.telegraf.password or "";
            };
          };
          kmonitor = if config.kuutamo.telegraf.kmonitoring_user_id == "" then { } else {
            http = {
              url = config.kuutamo.telegraf.kmonitoring_url;
              data_format = "prometheusremotewrite";
              username = "${config.kuutamo.telegraf.kmonitoring_protocol}-${config.kuutamo.telegraf.kmonitoring_user_id}";
              password = config.kuutamo.telegraf.kmonitoring_password;
            };
          };
        in
        {
          prometheus_client = {
            listen = ":9273";
            metric_version = 2;
          };
        } // monitor // kmonitor;
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
}
