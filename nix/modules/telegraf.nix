{ config, pkgs, lib, ... }:
let
  monitoring =
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
        };
        outputs = {
          prometheus_client = {
            listen = ":9273";
            metric_version = 2;
          };
        } // monitoring;
      };
    };
  };
}
