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
    services.telegraf = {
      enable = true;
      environmentFiles =
        if config.kuutamo.telegraf.hasMonitoring then [
          /var/lib/secrets/telegraf
          (pkgs.writeText "monitoring-configHash" config.kuutamo.telegraf.configHash)
        ] else [ ];
      extraConfig = {
        agent.interval = "60s";
        inputs = {
          prometheus.insecure_skip_verify = true;
          prometheus.urls = [
            "https://localhost:8080/_status/vars"
            "http://localhost:2233/metrics"
          ];
          outputs = lib.optionalAttrs config.kuutamo.telegraf.hasMonitoring {
            http = {
              url = "$MONITORING_URL";
              data_format = "prometheusremotewrite";
              username = "$MONITORING_USERNAME";
              password = "$MONITORING_PASSWORD";
            };
          };
        };
      };
    };
  };
}
