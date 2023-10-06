{ config, pkgs, lib, ... }:
let
  kld_metrics = if config.kuutamo ? kld then [ "http://localhost:2233/metrics" ] else [ ];
in
{
  options = {
    kuutamo.monitor.telegrafConfigHash = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "telegraf config hash";
    };
    kuutamo.monitor.telegrafHasMonitoring = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "has monitoring setting or not";
    };
    kuutamo.monitor.hostname = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "the hostname tag on metrics";
    };
    kuutamo.monitor.promtailClient = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "the endpoint to collect systemd journal. ie: http://kuutamo.monitor/loki/api/v1/push";
    };
  };
  config = {
    services.promtail = lib.mkIf (config.kuutamo.monitor.promtailClient != null) {
      enable = true;
      configuration = {
        server = {
          http_listen_port = 9080;
        };
        scrape_configs = [{
          job_name = "journal";
          journal = {
            max_age = "12h";
            labels = {
              job = "systemd-journal";
              inherit (config.kuutamo) hostname;
            };
          };
        }];
        clients = [{
          url = config.kuutamo.promtailClient;
        }];
      };
    };
    services.telegraf = {
      enable = true;
      environmentFiles =
        if config.kuutamo.monitor.telegrafHasMonitoring then [
          /var/lib/secrets/telegraf
          (pkgs.writeText "monitoring-configHash" config.kuutamo.monitor.telegrafConfigHash)
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
          ] ++ kld_metrics;
          prometheus.tags = {
            host = config.kuutamo.monitor.hostname;
          };
        };
        outputs = {
          prometheus_client = {
            # Not expose,
            # just for debug and let telegraf service running if not following monitoring settings
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
