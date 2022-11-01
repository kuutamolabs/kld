{ self, ... }:
{
  perSystem = { self', pkgs, ... }:
    {
      checks.format = pkgs.callPackage ./check-format.nix {
        inherit self;
        inherit (self'.devShells.default) formatters;
      };
      checks.test = self'.packages.lightning-knd.override {
        enableTests = true;
      };
      checks.lint = self'.packages.lightning-knd.override {
        enableLint = true;
      };
    };
}
