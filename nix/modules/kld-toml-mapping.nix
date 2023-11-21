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
    kuutamo.kld.logLevel = cfg.kld_log_level or "info";
    kuutamo.kld.nodeAlias = cfg.kld_node_alias or null;
    kuutamo.kld.publicAddresses = lib.optional (cfg ? hostname) "${cfg.hostname}:9234"
      # XXX
      # Check and ignore private ipv4 here
      # ++ lib.optional (cfg ? ipv4_address) "${cfg.ipv4_address}:9234"
      ++ lib.optional (cfg ? ipv6_address) "[${cfg.ipv6_address}]:9234";
    kuutamo.kld.nodeAliasColor = cfg.kld_node_alias_color or null;
    kuutamo.kld.apiIpAccessList = cfg.api_ip_access_list or [ ];
    kuutamo.kld.restApiPort = cfg.rest_api_port or 2244;
    kuutamo.kld.mnemonicPath = if (cfg ? kld_preset_mnemonic && cfg.kld_preset_mnemonic) then "/var/lib/secrets/mnemonic" else null;
    kuutamo.kld.probeInterval = cfg.probe_interval or 0;
    kuutamo.kld.probeAmtMSat = cfg.probe_amt_msat or 0;

    kuutamo.disko.disks = cfg.disks;
    kuutamo.disko.bitcoindDisks = cfg.bitcoind_disks;
    kuutamo.disko.networkInterface = cfg.network_interface or "eth0";
    kuutamo.disko.unlockKeys = cfg.public_ssh_keys;

    users.extraUsers.root.openssh.authorizedKeys.keys = if (cfg ? keep_root && cfg.keep_root) then cfg.public_ssh_keys else [ "" ];

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

    kuutamo.monitor.hostname = cfg.hostname;
    kuutamo.monitor.telegrafHasMonitoring = cfg.telegraf_has_monitoring or false;
    kuutamo.monitor.promtailHasClient = cfg.promtail_has_client or false;
    kuutamo.monitor.configHash = cfg.monitor_config_hash or "";

    kuutamo.upgrade.deploymentFlake = cfg.deployment_flake;

    # If the upgrade_schedule is not set, we will use the upgrade order and assume each upgrade can be done in 10 mins.
    kuutamo.upgrade.time = if (cfg ? upgrade_schedule) then cfg.upgrade_schedule else "*-*-* 2:${toString (cfg.upgrade_order or 0)}0:00";
  };
}
