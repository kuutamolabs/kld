{
  perSystem = { pkgs, ... }: {
    packages = {
      sensei = pkgs.callPackage ./sensei { };
    };
  };
}
