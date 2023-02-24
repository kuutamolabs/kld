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
          imports = [
            ./lightning-knd
            self.nixosModules.cockroachdb
          ];
          kuutamo.lightning-knd.package = packages.lightning-knd;
          services.bitcoind."lightning-knd-${config.kuutamo.lightning-knd.network}" = {
            package = packages.bitcoind;
          };
        };
      default = self.nixosModules.lightning-knd;

      cockroachdb = { pkgs, ... }: {
        imports = [ ./cockroachdb.nix ];
        services.cockroachdb.package = self.packages.${pkgs.hostPlatform.system}.cockroachdb;
      };

      disko-partitioning-script = ./disko-partitioning-script.nix;

      common-node = {
        imports = [
          inputs.srvos.nixosModules.server
          inputs.disko.nixosModules.disko
          self.nixosModules.disko-partitioning-script
          self.nixosModules.kuutamo-binary-cache
        ];
        system.stateVersion = "22.05";
      };

      cockroachdb-node.imports = [
        self.nixosModules.common-node
        self.nixosModules.cockroachdb
      ];

      lightning-knd-node.imports = [
        self.nixosModules.common-node
        self.nixosModules.lightning-knd
      ];
    };
  };
}
