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
          _module.args.self = self;
        };
      default = self.nixosModules.kld;

      cockroachdb = { pkgs, ... }: {
        imports = [ ./cockroachdb.nix ];
        kuutamo.cockroachdb.package = self.packages.${pkgs.hostPlatform.system}.cockroachdb;
      };

      kld-ctl = { pkgs, ... }: {
        config = {
          environment.systemPackages = [ self.packages.${pkgs.hostPlatform.system}.kld-ctl ];
        };
      };

      disko-partitioning-script = ./disko-partitioning-script.nix;

      common-node = {
        imports = [
          inputs.srvos.nixosModules.server
          inputs.disko.nixosModules.disko
          self.nixosModules.disko-partitioning-script
          self.nixosModules.kuutamo-binary-cache
          self.nixosModules.kld-ctl
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
        {
          kuutamo.kld.caPath = "/var/lib/secrets/kld/ca.pem";
          kuutamo.kld.certPath = "/var/lib/secrets/kld/kld.pem";
          kuutamo.kld.keyPath = "/var/lib/secrets/kld/kld.key";
          kuutamo.kld.cockroachdb.clientCertPath = "/var/lib/secrets/kld/client.kld.crt";
          kuutamo.kld.cockroachdb.clientKeyPath = "/var/lib/secrets/kld/client.kld.key";
          kuutamo.cockroachdb.rootClientCertPath = "/var/lib/secrets/cockroachdb/client.root.crt";
          kuutamo.cockroachdb.rootClientKeyPath = "/var/lib/secrets/cockroachdb/client.root.key";
        }
      ];
    };
  };
}
