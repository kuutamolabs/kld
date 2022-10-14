{ self, ... }:
{
  perSystem = { self', pkgs, ... }:
    {
      checks.format = pkgs.callPackage ./check-format.nix {
        inherit self;
        inherit (self'.devShells.default) formatters;
      };
      tests = pkgs.callPackage ./tests.nix {
        lightning-knd = self'.packages.lightning-knd;
      };
      lint = self'.packages.lightning-knd.override {
          enableLint = true;
      };
    };
}
