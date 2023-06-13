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
    kuutamo.telegraf.hostname = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "the hostname tag on metrics";
    };
  };
  config = {
    services.telegraf = {
      enable = true;
      environmentFiles =
        if config.kuutamo.telegraf.hasMonitoring then [
          /var/lib/secrets/telegraf
          (pkgs.writeText "monitoring-configHash" config.kuutamo.telegraf.configHash)
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
              host = config.kuutamo.telegraf.hostname;
            };
          };
          prometheus.insecure_skip_verify = true;
          prometheus.urls = [
            "https://${config.kuutamo.cockroachdb.http.address}:${toString config.kuutamo.cockroachdb.http.port}/_status/vars"
            "http://${config.kuutamo.kld.exporterAddress}/metrics"
          ];
          prometheus.tags = {
            host = config.kuutamo.telegraf.hostname;
          };
        };
        outputs = {
          prometheus_client = {
            # Not expose,
            # just for debug and let telegraf service running if not following monitoring settings
            listen = ":9273";
          };
          http = lib.mkIf config.kuutamo.telegraf.hasMonitoring {
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
