Our mission is to enable everyone to deploy and run resilient, secure and performant protocol infrastructure.

kuutamo is an open, turn-key, end-to-end solution for running best-in-class self-hosted nodes, anywhere.

In the world of software, you usually need to decide between using a managed SaaS or running everything yourself in a self-hosted environment, which means handling all the operations to keep things running smoothly. At kuutamo we believe that there is a better way. A hybrid cloud first way. A next generation cloud. Our packaged services can be deployed anywhere, to any cloud, bare metal, and to our users own infrastructure. We aim to provide all the updates, monitoring and ops tooling needed, along with world-class SRE for protocol and infrastructure support services.

# Lighting Router Node using kld from kuutamo

## Prerequisites

- Server(s)/node(s): Any Linux OS
- Workstation/development machine: Any Linux OS

These are two different machines. The kld manager, `kld-mgr` will run on your workstation. It will talk over SSH to your server/node. During install the server(s)/node(s) will be wiped and a fresh kuutamo lightning distribution will be installed.

## Server Setup

We have validated:

- [OVH](https://www.ovhcloud.com/en-gb/bare-metal/rise/rise-1/) - Rise 1, 32GB RAM, 2 x 4TB HDD + 2 x 500GB NVMe, with Ubuntu

Before [installing Ubuntu on the server](https://support.us.ovhcloud.com/hc/en-us/articles/115001775950-How-to-Install-an-OS-on-a-Dedicated-Server), [add your workstation SSH key](https://docs.ovh.com/gb/en/dedicated/creating-ssh-keys-dedicated/#importing-your-ssh-key-into-the-ovhcloud-control-panel_1).

## Workstation Setup

1. Install the Nix package manager, if you don't already have it. https://zero-to-nix.com/start/install is an excellent resource.

2. Enable `nix` command and [flakes](https://www.tweag.io/blog/2020-05-25-flakes/) features:

```bash
$ mkdir -p ~/.config/nix/ && printf 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```
3. Trust pre-built binaries (optional):

```bash
$ printf 'trusted-substituters = https://cache.garnix.io https://cache.nixos.org/\ntrusted-public-keys = cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g= cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY=' | sudo tee -a /etc/nix/nix.conf && sudo systemctl restart nix-daemon
```

4. Alias `kld-mgr` and use [`nix run`](https://determinate.systems/posts/nix-run) command:

```bash
$ printf 'alias kld-mgr="nix run --refresh github:kuutamolabs/lightning-knd --"' >> ~/.bashrc && source ~/.bashrc
```
5. Test the `kld-mgr` command:

```bash
$ kld-mgr --help
```

Answer ‘y’ to the four questions asked.
After some downloading you should see the help output.

```bash
Subcommand to run

Usage: kld-mgr [OPTIONS] <COMMAND>

Commands:
  generate-config  Generate NixOS configuration
  install          Install kld cluster on given hosts. This will remove all data of the current system!
  dry-update       Upload update to host and show which actions would be performed on an update
  update           Update hosts
  rollback         Rollback hosts to previous generation
  ssh              SSH into a host
  reboot           Reboot hosts
  help             Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>  configuration file to load [env: KLD_CONFIG=] [default: kld.toml]
      --yes              skip interactive dialogs by assuming the answer is yes
  -h, --help             Print help
  -V, --version          Print version
```

## Pro 3 node cluster install (v0.0.0-alpha5)

1. Create a new directory and in it put your kld.toml - copy the below and edit:

```toml
[global]
flake = "github:kuutamo/lightning-knd/0.0.0-alpha6"

[host_defaults]
public_ssh_keys = [
 "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAA you@computer",
]
bitcoind_disks = [
"/dev/sda",
"/dev/sdb",
]

[hosts.kld-00]
nixos_module = "kld-node"
ipv4_address = "1.1.1.1"
ipv4_cidr    = 24
ipv4_gateway = "1.1.1.254"

[hosts.db-00]
nixos_module = "cockroachdb-node"
ipv4_address = "2.2.2.2"
ipv4_cidr    = 24
ipv4_gateway = "2.2.2.254"

[hosts.db-01]
nixos_module = "cockroachdb-node"
ipv4_address = "3.3.3.3"
ipv4_cidr    = 24
ipv4_gateway = "3.3.3.254"
```

2. In this directory run:

```bash
$ kld-mgr install
```

3. After this install finishes you can connect to the node.

```bash
$ kld-mgr ssh
```

4. Follow the logs

```bash
[root@kld-00:~]$ journalctl -u kld.service
```

5. Run `kld-cli`

```bash
[root@kld-00:~]$ kld-cli --help
```
```
Usage: kld-cli --target <TARGET> --cert-path <CERT_PATH> --macaroon-path <MACAROON_PATH> <COMMAND>

Commands:
 get-info         Fetch information about this lightning node
 get-balance      Fetch confirmed and unconfirmed on-chain balance
 new-address      Generates new on-chain address for receiving funds
 withdraw         Send on-chain funds out of the wallet
 list-peers       Fetch a list of this nodes peers
 connect-peer     Connect with a network peer
 disconnect-peer  Disconnect from a network peer
 list-channels    Fetch a list of this nodes open channels
 open-channel     Open a channel with another node
 set-channel-fee  Set channel fees
 close-channel    Close a channel
 list-nodes       Get node information from the network graph
 help             Print this message or the help of the given subcommand(s)

Options:
 -t, --target <TARGET>                IP address or hostname of the target machine
 -c, --cert-path <CERT_PATH>          Path to the TLS cert of the target API
 -m, --macaroon-path <MACAROON_PATH>  Path to the macaroon for authenticating with the API
 -h, --help                           Print help
 -V, --version                        Print version

```

## Node upgrades

In the folder:

```bash
$ kld-mgr update
```

## Monitoring Settings

Although monitoring is not mandatory for deploying a node, it is highly recommended. By setting up monitoring, you can easily track metrics and identify potential issues.

To set up monitoring for your node, you can use the Kuutamo monitoring token. The process is simple - just obtain the token and configure your node to send metrics to the Kuutamo monitor. By default, the token file name is `kuutamo-monitoring.token`, and the default monitoring server is `https://mimir.monitoring-00-cluster.kuutamo.computer`.

If you have multiple hosts in `kld.toml` and want to use different tokens for each host, you can set the `kuutamo_monitoring_token_file` field for each host to point to the desired token file.

Alternatively, if you prefer to self-monitor the node, you can customize your monitor server by setting the `self_monitoring_url`, `self_monitoring_username`, and `self_monitoring_password` fields of the host. The `self_monitoring_url` should implement [Prometheus's Remote Write API](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write).

## Further Information

- [kld v0.0.1-alpha6 install guide Google slides](https://docs.google.com/presentation/d/1MfzXU3pHnGyMZFql3ga00lrOpOwLCJB8Nq0-CEsyn9U)

