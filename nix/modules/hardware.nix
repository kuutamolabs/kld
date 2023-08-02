{ lib, config, ... }: {
  # Upstream this?
  options.kuutamo.disko.disks = lib.mkOption {
    type = lib.types.listOf lib.types.path;
    default = [ "/dev/nvme0n1" "/dev/nvme1n1" ];
    description = lib.mdDoc "Disks formatted by disko";
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

    # / is a mirror raid
    # boot.loader.grub.devices = config.kuutamo.disko.disks;

    # Enable raid support specifically, this will disable srvos's
    # systemd-initrd as well, which currently is not compatible with mdraid.
    boot.swraid.enable = true;
    systemd.services.mdmonitor.enable = false;

    # for mdraid 1.1
    # boot.loader.grub.extraConfig = "insmod mdraid1x";
    boot.loader.grub.enable = true;
    boot.loader.grub.efiSupport = true;
    boot.loader.grub.efiInstallAsRemovable = true;

    # initrd networking for unlocking LUKS
    boot.initrd.network = {
      enable = true;
      ssh = {
        enable = true;
        port = 2222;
        authorizedKeys = config.users.extraUsers.root.openssh.authorizedKeys.keys;
        hostKeys = [
          "/var/lib/secrets/sshd_key"
        ];
      };
      postCommands = ''
        ip link set dev eth0 up

        ${lib.optionalString (config.kuutamo.network.ipv4.address != null) ''
          ip addr add ${config.kuutamo.network.ipv4.address}/${builtins.toString config.kuutamo.network.ipv4.cidr} dev eth0
          ip route add ${config.kuutamo.network.ipv4.gateway} dev eth0
          ip route add default via ${config.kuutamo.network.ipv4.gateway} dev eth0
        ''}
        ${lib.optionalString (config.kuutamo.network.ipv6.address != null) ''
          ip -6 addr add ${config.kuutamo.network.ipv6.address}/${builtins.toString config.kuutamo.network.ipv6.cidr} dev eth0
          ip -6 route add ${config.kuutamo.network.ipv6.gateway} dev eth0
          ip -6 route add default via ${config.kuutamo.network.ipv6.gateway} dev eth0
        ''}
      '';

    };
  };
}
