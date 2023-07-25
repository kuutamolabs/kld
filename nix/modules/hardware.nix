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
    ];
    # / is a mirror raid
    boot.loader.grub.devices = config.kuutamo.disko.disks;

    # Enable raid support specifically, this will disable srvos's
    # systemd-initrd as well, which currently is not compatible with mdraid.
    boot.swraid.enable = true;
    systemd.services.mdmonitor.enable = false;

    # for mdraid 1.1
    boot.loader.grub.extraConfig = "insmod mdraid1x";
    boot.loader.grub.enable = true;
    boot.loader.grub.efiSupport = true;
    boot.loader.grub.efiInstallAsRemovable = true;

    # initrd networking for unlocking LUKS
    boot.initrd.network = {
      enable = true;
      ssh = {
        enable = true;
        authorizedKeys = config.users.extraUsers.root.openssh.authorizedKeys.keys;
        port = 22;
        hostKeys = [
          "/var/lib/secrets/disk_encryption/rsa.key"
          "/var/lib/secrets/disk_encryption/ed25519.key"
        ];
      };
    };
    boot.kernelParams = [
      "ip=${if config.kuutamo.network.ipv4.address == null then "dhcp" else config.kuutamo.network.ipv4.address}"
    ];
  };
}
