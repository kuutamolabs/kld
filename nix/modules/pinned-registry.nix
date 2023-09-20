{ inputs, ... }:
_:
{
  nix.registry = {
    nixpkgs.to = {
      type = "path";
      path = inputs.nixpkgs;
    };
  };
}
