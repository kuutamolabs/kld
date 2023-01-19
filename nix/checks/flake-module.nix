_:
{
  perSystem = { self', ... }:
    {
      checks.test = self'.packages.lightning-knd.override {
        enableTests = true;
      };
      checks.lint = self'.packages.lightning-knd.override {
        enableLint = true;
      };
    };
}
