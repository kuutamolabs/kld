{ config, pkgs, lib, ... }:
let
  kld_metrics = if config.kuutamo ? kld then [ "http://localhost:2233/metrics" ] else [ ];
  prettyJSON = conf: pkgs.runCommandLocal "promtail-config.json" { } ''
    echo '${builtins.toJSON conf}' | ${pkgs.buildPackages.jq}/bin/jq 'del(._module)' > $out
  '';
in
{
  options = {
    kuutamo.monitor.configHash = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "config hash for telegraf and promtail";
    };
    kuutamo.monitor.telegrafHasMonitoring = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "has telegraf monitoring setting or not";
    };
    kuutamo.monitor.hostname = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "the hostname tag on metrics";
    };
    kuutamo.monitor.promtailHasClient = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "has promtail client setting or not";
    };
  };
  config = {
    environment.systemPackages = lib.optional config.kuutamo.monitor.promtailHasClient pkgs.promtail;
    systemd.services.promtail = lib.mkIf config.kuutamo.monitor.promtailHasClient {
      description = "Promtail log ingress";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      stopIfChanged = false;

      serviceConfig = {
        Restart = "on-failure";
        TimeoutStopSec = 10;
        EnvironmentFile = /var/lib/secrets/promtail;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
        PrivateMounts = true;
        CacheDirectory = "promtail";
        ReadWritePaths = [ "/var/cache/promtail" ];
        ExecStart = "${pkgs.promtail}/bin/promtail -config.expand-env=true -config.file=${prettyJSON {
          server = {
            http_listen_port = 9080;
          };
          positions = {
            filename = "/var/cache/promtail/positions.yaml";
          };
          scrape_configs = [{
            job_name = "journal";
            journal = {
              json = false;
              path = "/var/log/journal";
              max_age = "12h";
              labels = {
                job = "systemd-journal";
                host = config.kuutamo.monitor.hostname;
              };
            };
            relabel_configs = [{
              source_labels = ["__journal__systemd_unit"];
              target_label = "unit";
            }];
          }];
          clients = [{
            url = "\${CLIENT_URL}";
          }];
        }}";

        User = "promtail";
        Group = "promtail";

        CapabilityBoundingSet = "";
        NoNewPrivileges = true;

        ProtectKernelModules = true;
        SystemCallArchitectures = "native";
        ProtectKernelLogs = true;
        ProtectClock = true;

        LockPersonality = true;
        ProtectHostname = true;
        RestrictRealtime = true;
        MemoryDenyWriteExecute = true;
        PrivateUsers = true;

        SupplementaryGroups = [ "systemd-journal" ];
      };
    };

    users.groups.promtail = { };
    users.users.promtail = {
      description = "Promtail service user";
      isSystemUser = true;
      group = "promtail";
    };
    services.telegraf = {
      enable = true;
      environmentFiles =
        if config.kuutamo.monitor.telegrafHasMonitoring then [
          /var/lib/secrets/telegraf
          (pkgs.writeText "monitoring-configHash" config.kuutamo.monitor.configHash)
        ] else [ ];
      extraConfig = {
        agent.interval = "60s";
        agent.round_interval = true;
        agent.metric_batch_size = 10000;
        agent.collection_offset = "5s";
        agent.flush_interval = "60s";
        agent.flush_jitter = "40s";
        inputs = {
          cpu = {
            tags = {
              host = config.kuutamo.monitor.hostname;
            };
          };
          prometheus.insecure_skip_verify = true;
          prometheus.urls = [
            "https://${config.kuutamo.cockroachdb.http.address}:${toString config.kuutamo.cockroachdb.http.port}/_status/vars"
          ] ++ kld_metrics ++ lib.optional config.kuutamo.monitor.promtailHasClient "http://127.0.0.1:9080/metrics";
          prometheus.tags = {
            host = config.kuutamo.monitor.hostname;
          };
        };
        outputs = {
          prometheus_client = {
            # This port is not exposed to the outside world, only used
            # by CI to check that the metrics are being generated.
            listen = ":9273";
          };
          http = lib.mkIf config.kuutamo.monitor.telegrafHasMonitoring {
            url = "$MONITORING_URL";
            data_format = "prometheusremotewrite";
            username = "$MONITORING_USERNAME";
            password = "$MONITORING_PASSWORD";
          };
        };
      };
    };
  };
}
