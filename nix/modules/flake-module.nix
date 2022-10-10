{ self, ... }:
{
  flake = { ... }: {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      lightning-knd = ./lightning-knd;
      default = self.nixosModules.lightning-knd;
    };
  };
}
