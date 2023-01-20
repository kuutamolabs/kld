{ self, ... }:
{
  flake = {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      lightning-knd = { pkgs, ... }: {
        imports = [ ./lightning-knd ];
        kuutamo.lightning-knd.package = self.packages.${pkgs.hostPlatform.system}.lightning-knd;
        services.cockroachdb.package = self.packages.${pkgs.hostPlatform.system}.cockroachdb;
      };
      default = self.nixosModules.lightning-knd;
    };
  };
}
