{ self, lib, ... }:

{
  perSystem = { pkgs, ... }: {
    checks = lib.optionalAttrs pkgs.stdenv.isLinux {
      lightning-knd = import ./lightning-knd.nix { inherit self pkgs; };
    };
  };
}
