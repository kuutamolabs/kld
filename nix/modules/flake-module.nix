{ self, ... }:
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
    };
  };
}
