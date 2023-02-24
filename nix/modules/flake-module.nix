{ self, inputs, ... }:
{
  flake = {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      lightning-knd = { config, pkgs, ... }:
        let
          packages = self.packages.${pkgs.hostPlatform.system};
        in
        {
          imports = [ ./lightning-knd ];
          kuutamo.lightning-knd.package = packages.lightning-knd;
          services.cockroachdb.package = packages.cockroachdb;
          services.bitcoind."lightning-knd-${config.kuutamo.lightning-knd.network}" = {
            package = packages.bitcoind;
          };
        };
      default = self.nixosModules.lightning-knd;

      disko-partitioning-script = ./disko-partitioning-script.nix;

      lightning-knd-node = {
        imports = [
          ./lightning-knd-node
          self.nixosModules.lightning-knd
          self.nixosModules.disko-partitioning-script
          self.nixosModules.kuutamo-binary-cache
          inputs.srvos.nixosModules.server
          inputs.disko.nixosModules.disko
        ];
      };
    };
  };
}
