{ self, inputs, ... }:
{
  flake = {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      kld = { pkgs, ... }:
        let
          packages = self.packages.${pkgs.hostPlatform.system};
        in
        {
          imports = [
            ./kld
            self.nixosModules.cockroachdb
          ];
          kuutamo.kld.package = packages.kld;
        };
      default = self.nixosModules.kld;

      bitcoind = { pkgs, ... }: {
        imports = [ ./bitcoind.nix ];
        kuutamo.bitcoind.package = self.packages.${pkgs.hostPlatform.system}.bitcoind;
      };
      electrs = { ... }: {
        imports = [ ./electrs.nix ];
      };
      cockroachdb = { pkgs, ... }: {
        imports = [ ./cockroachdb.nix ];
        kuutamo.cockroachdb.package = self.packages.${pkgs.hostPlatform.system}.cockroachdb;
      };

      telegraf.imports = [
        inputs.srvos.nixosModules.mixins-telegraf
        ./telegraf.nix
      ];

      disko-partitioning-script = ./disko-partitioning-script.nix;

      kld-ctl = { pkgs, ... }:
        let
          packages = self.packages.${pkgs.hostPlatform.system};
        in
        {
          imports = [ ./ctl ];
          kuutamo.ctl.package = packages.kld-ctl;
        };

      common-node = {
        imports = [
          inputs.srvos.nixosModules.server
          inputs.disko.nixosModules.disko
          self.nixosModules.disko-partitioning-script
          self.nixosModules.kuutamo-binary-cache
          self.nixosModules.kld-ctl
          ./hardware.nix
          ./network.nix
          ./upgrade.nix
          ./telegraf.nix
          (import ./pinned-registry.nix { inherit inputs; })
        ];
        system.stateVersion = "22.05";
        _module.args.self = self;
      };

      cockroachdb-node.imports = [
        ./db-toml-mapping.nix
        self.nixosModules.common-node
        self.nixosModules.cockroachdb
      ];

      kld-node.imports = [
        ./kld-toml-mapping.nix
        self.nixosModules.common-node
        self.nixosModules.bitcoind
        self.nixosModules.electrs
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
