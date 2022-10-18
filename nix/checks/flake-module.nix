{ self, ... }:
{
  perSystem = { self', pkgs, ... }:
    {
      checks.format = pkgs.callPackage ./check-format.nix {
        inherit self;
        inherit (self'.packages) treefmt;
      };
      checks.test = self'.packages.lightning-knd.override {
        enableTests = true;
      };
      checks.lint = self'.packages.lightning-knd.override {
        enableLint = true;
      };
    };
}
