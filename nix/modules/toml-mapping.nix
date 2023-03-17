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

    users.extraUsers.root.openssh.authorizedKeys.keys = cfg.public_ssh_keys;

    kuutamo.network.macAddress = cfg.mac_address or null;

    kuutamo.network.ipv4.address = cfg.ipv4_address;
    kuutamo.network.ipv4.gateway = cfg.ipv4_gateway;
    kuutamo.network.ipv4.cidr = cfg.ipv4_cidr;

    kuutamo.network.ipv6.address = cfg.ipv6_address or null;
    kuutamo.network.ipv6.gateway = cfg.ipv6_gateway or null;
    kuutamo.network.ipv6.cidr = cfg.ipv6_cidr or 128;

    kuutamo.kld.caFile = "/var/lib/secrets/kld/ca.pem";
    kuutamo.kld.certFile = "/var/lib/secrets/kld/kld.pem";
    kuutamo.kld.keyFile = "/var/lib/secrets/kld/kld.key";

    kuutamo.kld.cockroachdb.clientCertPath = "/var/lib/secrets/kld/client.kld.crt";
    kuutamo.kld.cockroachdb.clientKeyPath = "/var/lib/secrets/kld/client.kld.key";
  };
}
