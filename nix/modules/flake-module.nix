{ self, inputs, ... }:
{
  flake = {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      kld = { config, pkgs, ... }:
        let
          packages = self.packages.${pkgs.hostPlatform.system};
        in
        {
          imports = [
            ./kld
            self.nixosModules.cockroachdb
          ];
          kuutamo.kld.package = packages.kld;
          services.bitcoind."kld-${config.kuutamo.kld.network}" = {
            package = packages.bitcoind;
          };
        };
      default = self.nixosModules.kld;

      cockroachdb = { pkgs, ... }: {
        imports = [ ./cockroachdb.nix ];
        kuutamo.cockroachdb.package = self.packages.${pkgs.hostPlatform.system}.cockroachdb;
      };

      disko-partitioning-script = ./disko-partitioning-script.nix;

      common-node = {
        imports = [
          inputs.srvos.nixosModules.server
          inputs.disko.nixosModules.disko
          self.nixosModules.disko-partitioning-script
          self.nixosModules.kuutamo-binary-cache
          ./toml-mapping.nix
          ./hardware.nix
          ./network.nix
        ];
        system.stateVersion = "22.05";
      };

      cockroachdb-node.imports = [
        self.nixosModules.common-node
        self.nixosModules.cockroachdb
      ];

      kld-node.imports = [
        self.nixosModules.common-node
        self.nixosModules.kld
      ];
    };
  };
}
