{ config
, lib
, ...
}:
let
  boot = {
    size = "1M";
    type = "EF02";
  };

  ESP = {
    name = "ESP";
    size = "500M";
    type = "EF00";
    content = {
      type = "mdraid";
      name = "boot";
    };
  };

  raid-root = {
    size = "100%";
    content = {
      type = "mdraid";
      name = "root";
    };
  };
in
{
  disko.devices = {
    disk = lib.genAttrs config.kuutamo.disko.disks
      (disk: {
        type = "disk";
        device = disk;
        content = {
          type = "gpt";
          partitions = {
            inherit boot ESP raid-root;
          };
        };
      }) // lib.genAttrs config.kuutamo.disko.bitcoindDisks (disk: {
      type = "disk";
      device = disk;
      content = {
        type = "mdraid";
        name = "bitcoind";
      };
    });

    mdadm = {
      boot = {
        type = "mdadm";
        level = 1;
        # metadata 1.0 so we can use it as an esp partition
        metadata = "1.0";
        content = {
          type = "filesystem";
          format = "vfat";
          mountpoint = "/boot";
        };
      };
      root = {
        type = "mdadm";
        level = 1;
        content = {
          type = "luks";
          name = "root-encrypted";
          keyFile = "/var/lib/disk_encryption_key";
          settings.preLVM = false;
          content = {
            type = "filesystem";
            format = "ext4";
            mountpoint = "/";
          };
        };
      };
      bitcoind = lib.mkIf (config.kuutamo.disko.bitcoindDisks != [ ]) {
        type = "mdadm";
        level = 1;
        content = {
          type = "filesystem";
          format = "ext4";
          mountpoint = config.kuutamo.disko.bitcoindDataDir;
        };
      };
    };
  };
}
