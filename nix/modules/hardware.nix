{ lib, config, ... }: {
  # Upstream this?
  options.kuutamo.disko.disks = lib.mkOption {
    type = lib.types.listOf lib.types.path;
    default = [ "/dev/nvme0n1" "/dev/nvme1n1" ];
    description = lib.mdDoc "Disks formatted by disko";
  };

  options.kuutamo.disko.networkInterface = lib.mkOption {
    type = lib.types.str;
    default = "eth0";
    description = lib.mdDoc "The network interface for internet";
  };
  options.kuutamo.disko.unlockKeys = lib.mkOption {
    type = lib.types.listOf lib.types.str;
    default = [ ];
    description = lib.mdDoc "Ssh key to login locked machines";
  };

  imports = [
    ./raid-config.nix
    ./bitcoind-disks.nix
  ];

  config = {
    boot.initrd.availableKernelModules = [
      "xhci_pci"
      "ahci"
      "nvme"
      "r8169"
      "ixgbe"
      "igc" # 2.5GbitE adapter
      "cdc_ether"
      "virtio_net"
      "virtio_pci"
      "virtio_mmio"
      "virtio_blk"
      "virtio_scsi"
      "9p"
      "9pnet_virtio"
    ];
    # XXX on some platforms we pick up the wrong console. In this cases we default to serial,
    # also our ipmi has VGA output, uncomment the line below.
    srvos.boot.consoles = lib.mkDefault [ ];

    # Enable raid support specifically, this will disable srvos's
    # systemd-initrd as well, which currently is not compatible with mdraid.
    boot.swraid.enable = true;
    systemd.services.mdmonitor.enable = false;

    boot.loader.grub.enable = true;
    boot.loader.grub.efiSupport = true;
    boot.loader.grub.efiInstallAsRemovable = true;

    # # initrd networking for unlocking LUKS
    # boot.initrd.network = {
    #   enable = true;
    #   ssh = {
    #     enable = true;
    #     port = 2222;
    #     authorizedKeys = config.kuutamo.disko.unlockKeys;
    #     hostKeys = [
    #       "/var/lib/secrets/sshd_key"
    #     ];
    #   };
    #   postCommands = ''
    #     ip link set dev ${config.kuutamo.disko.networkInterface} up
    #     ${lib.optionalString (config.kuutamo.network.ipv4.address != null) ''
    #       ip addr add ${config.kuutamo.network.ipv4.address}/${builtins.toString config.kuutamo.network.ipv4.cidr} dev ${config.kuutamo.disko.networkInterface}
    #       ip route add ${config.kuutamo.network.ipv4.gateway} dev ${config.kuutamo.disko.networkInterface}
    #       ip route add default via ${config.kuutamo.network.ipv4.gateway} dev ${config.kuutamo.disko.networkInterface}
    #     ''}
    #     ${lib.optionalString (config.kuutamo.network.ipv6.address != null) ''
    #       ip -6 addr add ${config.kuutamo.network.ipv6.address}/${builtins.toString config.kuutamo.network.ipv6.cidr} dev ${config.kuutamo.disko.networkInterface}
    #       ip -6 route add ${config.kuutamo.network.ipv6.gateway} dev ${config.kuutamo.disko.networkInterface}
    #       ip -6 route add default via ${config.kuutamo.network.ipv6.gateway} dev ${config.kuutamo.disko.networkInterface}
    #     ''}
    #   '';
    # };
    # boot.initrd.luks.devices.root-encrypted.fallbackToPassword = true;
    # boot.initrd.luks.devices.root-encrypted.keyFile = "/key-file";
    # boot.initrd.postDeviceCommands = ''
    #   set -x
    #   if cat /proc/cmdline | grep 'disk-key'; then
    #     echo "Unlock from kernel command"
    #     key=$(cat /proc/cmdline | sed -e 's/^.*disk-key=//')
    #     echo -n "$key" > /key-file;
    #   fi
    # '';
  };
}
