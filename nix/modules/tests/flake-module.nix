{ self, lib, ... }:

{
  perSystem = { pkgs, ... }: {
    checks = lib.optionalAttrs pkgs.stdenv.isLinux {
      kld = import ./kld.nix { inherit self pkgs; };
    };
  };
}
