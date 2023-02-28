{ self, inputs, lib, ... }:
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
          ./hardware.nix
          ./network.nix
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
    nixosConfigurations =
      let
        dummyConfig = {
          kuutamo.network.ipv6.address = "2001:db8::1";
          kuutamo.network.ipv6.cidr = 64;
          kuutamo.network.ipv6.gateway = "fe80::1";
          users.users.root.openssh.authorizedKeys.keys = [
            "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
          ];
        };
      in
      {
        # some example configuration to make it eval
        example-lnd-node = lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            self.nixosModules.lightning-knd-node
            dummyConfig
          ];
          specialArgs = { inherit self; };
        };
        example-cockroach-node = lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            self.nixosModules.cockroachdb-node
            dummyConfig
          ];
          specialArgs = { inherit self; };
        };
      };
  };
}
