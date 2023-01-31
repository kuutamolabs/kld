{ self, inputs, ... }: {
  perSystem = { config, self', pkgs, system, ... }: {
    packages = {
      lightning-knd = pkgs.callPackage ./lightning-knd.nix {
        inherit self;
        craneLib = inputs.crane.lib.${system};
        inherit (config.packages) cockroachdb;
      };
      remote-pdb = pkgs.python3.pkgs.callPackage ./remote-pdb.nix { };
      bitcoind = pkgs.bitcoind.override { withGui = false; };
      cockroachdb = pkgs.callPackage ./cockroachdb.nix { };
      default = self'.packages.lightning-knd;
    };
  };
}
