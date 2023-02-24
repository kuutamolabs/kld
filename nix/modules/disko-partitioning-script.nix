{ config, pkgs, lib, ... }:
{
  # FIXME disko still sometimes fail if the disk layout changes and old raids are still present
  # Therefore we have our own logic to remove old raids/partitions.
  system.build.disko = lib.mkForce (pkgs.writeScript "disko" ''
    #!/usr/bin/env bash
    set -eux
    # make partitioning idempotent by dismounting already mounted filesystems
    if findmnt /mnt; then
      umount -Rlv /mnt
    fi
    # stop all existing raids
    shopt -s nullglob

    for r in /dev/md/* /dev/md[0-9]*; do
      # might fail if the device was already closed
      mdadm --stop "$r" || true
    done

    for p in /dev/nvme[0-9]*n1 /dev/sd[a-z]; do
      blkdiscard "$p" || true;
    done
    ${config.system.build.diskoNoDeps}
  '');
}
