{ config, lib, pkgs, ... }:

let
  cfg = config.kuutamo.deployConfig;
  settingsFormat = pkgs.formats.toml { };
in
{
  options.kuutamo.deployConfig = lib.mkOption {
    default = { };
    description = lib.mdDoc "toml configuration from kld-mgr cli";
    inherit (settingsFormat) type;
  };

  # deployConfig is optional
  config = lib.mkIf (cfg != { }) {
    networking.hostName = cfg.name;
    kuutamo.cockroachdb.nodeName = cfg.name;

    kuutamo.disko.disks = cfg.disks;
    kuutamo.disko.bitcoindDisks = cfg.bitcoind_disks;
    kuutamo.disko.networkInterface = cfg.network_interface or "eth0";

    users.extraUsers.root.openssh.authorizedKeys.keys = cfg.public_ssh_keys;

    kuutamo.network.macAddress = cfg.mac_address or null;

    kuutamo.network.ipv4.address = cfg.ipv4_address or null;
    kuutamo.network.ipv4.gateway = cfg.ipv4_gateway or null;
    kuutamo.network.ipv4.cidr = cfg.ipv4_cidr or 32;

    kuutamo.network.ipv6.address = cfg.ipv6_address or null;
    kuutamo.network.ipv6.gateway = cfg.ipv6_gateway or null;
    kuutamo.network.ipv6.cidr = cfg.ipv6_cidr or 128;

    kuutamo.cockroachdb.caCertPath = "/var/lib/secrets/cockroachdb/ca.crt";
    kuutamo.cockroachdb.nodeCertPath = "/var/lib/secrets/cockroachdb/node.crt";
    kuutamo.cockroachdb.nodeKeyPath = "/var/lib/secrets/cockroachdb/node.key";

    networking.extraHosts = lib.concatMapStringsSep "\n"
      (peer: ''
        ${lib.optionalString (peer ? ipv4_address && peer.ipv4_address != null) "${peer.ipv4_address} ${peer.name}"}
        ${lib.optionalString (peer ? ipv6_address && peer.ipv6_address != null) "${peer.ipv6_address} ${peer.name}"}
      '')
      cfg.cockroach_peers;

    kuutamo.cockroachdb.join = lib.optionals ((builtins.length cfg.cockroach_peers) > 1) (builtins.map (peer: peer.name) cfg.cockroach_peers);

    kuutamo.monitor.hostname = cfg.ssh_hostname;
    kuutamo.monitor.telegrafHasMonitoring = cfg.telegraf_has_monitoring or false;
    kuutamo.monitor.promtailHasClient = cfg.promtail_has_client or false;
    kuutamo.monitor.configHash = cfg.monitor_config_hash or "";

    kuutamo.upgrade.deploymentFlake = cfg.deployment_flake;
    # Assume the upgrade can be done in 10 mins, so the node will not upgrade at the same time
    kuutamo.upgrade.time = "*-*-* 2:${toString (cfg.upgrade_order or 0)}0:00";
  };
}
