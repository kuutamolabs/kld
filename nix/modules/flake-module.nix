{ self, ... }:
{
  flake = { ... }: {
    nixosModules = {
      kuutamo-binary-cache = ./binary-cache;
      lightning-knd = ./lightning-knd;
      cockroach = ./cockroach;
      default = self.nixosModules.lightning-knd;
    };
  };
}
