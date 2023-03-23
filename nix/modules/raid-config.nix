{ config
, lib
, ...
}:
let
  biosBoot = {
    type = "partition";
    start = "0MB";
    end = "1MB";
    name = "boot";
    flags = [ "bios_grub" ];
  };

  efiBoot = {
    type = "partition";
    name = "ESP";
    start = "1MB";
    end = "500MB";
    bootable = true;
    content = {
      type = "mdraid";
      name = "boot";
    };
  };

  raidPart = {
    type = "partition";
    name = "raid-root";
    start = "500MB";
    end = "100%";
    bootable = true;
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
          type = "table";
          format = "gpt";
          partitions = [
            biosBoot
            efiBoot
            raidPart
          ];
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
          type = "filesystem";
          format = "ext4";
          mountpoint = "/";
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
