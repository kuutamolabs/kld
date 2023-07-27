kuutamo is an open, turn-key, end-to-end solution for running best-in-class self-hosted nodes, anywhere.

In the world of software, you usually need to decide between using a managed SaaS or running everything yourself in a self-hosted environment, which means handling all the operations to keep things running smoothly. At kuutamo we believe that there is a better way. A hybrid cloud first way. A next generation cloud. Our packaged services can be deployed anywhere, to any cloud, bare metal, and to our users own infrastructure. We aim to provide all the updates, monitoring and operations tooling needed, along with world-class SRE support for protocol and infrastructure services.

# Lighting Service Provider (LSP) node cluster

**Nota bene**: kuutamo is bleeding edge decentralized financial infrastructure. Use with caution and only with funds you are prepared to lose.
If you want to put it into production and would like to discuss SRE overlay support, please get in touch with us at [opencore-support@kuutamo.co](mailto:opencore-support@kuutamo.co)

## Prerequisites

- 1 or 3 server(s)/node(s): Any Linux OS
- 1 workstation/local machine: Any Linux OS, MacOS.
- Nix

## Components

- `kld-mgr` - A CLI tool that will SSH to your server(s) to perform the initial deployment and support ongoing infrastructure operations (e.g. upgrades)
- `kld-cli` - A CLI tool that will talk to the `kld` API to support LSP operations (e.g. channel open)
- `kld` - kuutamo lightning daemon - our LSP router node software, built on [LDK](https://github.com/lightningdevkit)
- `cockroachdb` - Cockroach DB - a cloud-native, distributed SQL database
- `telegraf` - an agent for collecting and sending metrics to any URL that supports the [Prometheus's Remote Write API](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write)

The server(s) will run `kld` and `cockroachdb`.   
The local machine will run `kld-mgr`. `kld-mgr` requires root access to server(s); therefore in production, this should be executed on a hardened, trusted machine.   
`kld-cli` is also available on the server(s), and can be run on the local machine.

## Nix quickstart

kld-mgr:
```bash
nix run github:kuutamolabs/lightning-knd#kld-mgr -- help
```

kld-cli:
```bash
nix run github:kuutamolabs/lightning-knd#kld-cli -- help
```

## Example server hardware setup

- [OVH](https://www.ovhcloud.com/en-gb/bare-metal/rise/rise-1/) - Rise 1, 32GB RAM, 2 x 4TB HDD + 2 x 500GB NVMe, with Ubuntu

Before [installing Ubuntu on the server](https://support.us.ovhcloud.com/hc/en-us/articles/115001775950-How-to-Install-an-OS-on-a-Dedicated-Server), [add your workstation SSH key](https://docs.ovh.com/gb/en/dedicated/creating-ssh-keys-dedicated/#importing-your-ssh-key-into-the-ovhcloud-control-panel_1).

## workstation/local machine setup

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
After some downloading, you should see the help output.

```
$ nix run github:kuutamolabs/lightning-knd#kld-mgr -- help
Subcommand to run

Usage: kld-mgr [OPTIONS] <COMMAND>

Commands:
  generate-config   Generate NixOS configuration
  generate-example  Generate kld.toml example
  install           Install kld cluster on given hosts. This will remove all data of the current system!
  dry-update        Upload update to host and show which actions would be performed on an update
  update            Update hosts
  rollback          Rollback hosts to previous generation
  ssh               SSH into a host
  reboot            Reboot hosts
  system-info       Get system info from a host
  help              Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>  configuration file to load [env: KLD_CONFIG=] [default: kld.toml]
      --yes              skip interactive dialogs by assuming the answer is yes
  -h, --help             Print help
  -V, --version          Print version

```

## 3 server cluster

1. Create a new directory, and in it, put your `kld.toml` file - Copy and edit the minimal template below to get started:

```toml
[global]
flake = "github:kuutamo/lightning-knd/"

[host_defaults]
public_ssh_keys = [
 "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAA you@computer",
]

# The example RISE1 server in OVH, deployed with a Ubuntu image
# will be configured with 'ubuntu' as the admin user
install_ssh_user = "ubuntu"

# This allows us to specify different disks for the bitcoin datebase.
# The example RISE1 server in OVH has 2x4TB HDD and 2x500GB NVMe
# Here we specify the bitcoin DB should be on the 4TB disks.
bitcoind_disks = [
"/dev/sda",
"/dev/sdb",
]

# The `kld-node` module contains both `kld` and `cockroachdb`
# Currently each custer can support a maximum of 1 `kld-node`
[hosts.kld-00]
nixos_module = "kld-node"
ipv4_address = "1.1.1.1"
ipv4_cidr    = 24
ipv4_gateway = "1.1.1.254"

# Optionally, each cluster can support 2 additional database nodes.
# The `cockroachdb-node` module only contains `cockroachdb`
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
$ nix run github:kuutamolabs/lightning-knd#kld-cli -- help
Usage: kld-cli --target <TARGET> --cert-path <CERT_PATH> --macaroon-path <MACAROON_PATH> <COMMAND>

Commands:
  get-info                    Fetch information about this lightning node
  sign                        Creates a signature of the message using node\'s secret key (message limit 65536 chars)
  get-balance                 Fetch confirmed and unconfirmed on-chain balance
  new-address                 Generates new on-chain address for receiving funds
  withdraw                    Send on-chain funds out of the wallet
  list-funds                  Show available funds from the internal wallet
  list-peers                  Fetch a list of this nodes peers
  connect-peer                Connect with a network peer
  disconnect-peer             Disconnect from a network peer
  list-channels               Fetch a list of this nodes open channels
  open-channel                Open a channel with another node
  set-channel-fee             Set channel fees
  close-channel               Close a channel
  network-nodes               Get node information from the network graph
  network-channels            Get channel information from the network graph
  fee-rates                   Return feerate estimates, either satoshi-per-kw or satoshi-per-kb
  keysend                     Pay a node without an invoice
  generate-invoice            Generate a bolt11 invoice for receiving a payment
  list-invoices               List all invoices
  pay-invoice                 Pay an invoice
  list-payments               List all payments
  estimate-channel-liquidity  Esimate channel liquidity to a target node
  local-remote-balance        Fetch the aggregate local and remote channel balances (msat) of the node
  get-fees                    Get node routing fees
  list-forwards               Fetch a list of the forwarded htlcs
  help                        Print this message or the help of the given subcommand(s)

Options:
  -t, --target <TARGET>                IP address or hostname of the target machine
  -c, --cert-path <CERT_PATH>          Path to the TLS cert of the target API
  -m, --macaroon-path <MACAROON_PATH>  Path to the macaroon for authenticating with the API
  -h, --help                           Print help
  -V, --version                        Print version

```

## One command upgrades

In the folder:

```bash
$ kld-mgr update
```

## Monitoring Settings

Although monitoring is not mandatory for deploying a node, it is highly recommended.
Configure the `self_monitoring_url`, `self_monitoring_username`, and `self_monitoring_password` fields of the host in the kld.toml.
