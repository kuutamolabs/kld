{ lib, ... }:

{
  options.kuutamo.disko.bitcoindDisks = lib.mkOption {
    type = lib.types.listOf lib.types.path;
    default = [ ];
    description = lib.mdDoc "Disks formatted by disko for bitcoind";
  };
  options.kuutamo.disko.bitcoindDataDir = lib.mkOption {
    type = lib.types.path;
    default = "/var/lib/bitcoind";
    description = lib.mdDoc "Disks formatted by disko for bitcoind";
  };

}
