{ self, inputs, ... }: {
  perSystem = { config, self', pkgs, system, ... }: {
    packages = {
      kld = pkgs.callPackage ./kld.nix {
        inherit self;
        craneLib = inputs.crane.lib.${system};
        inherit (config.packages) cockroachdb;
      };
      kld-deploy = pkgs.callPackage ./kld-deploy.nix {
        inherit self;
        craneLib = inputs.crane.lib.${system};
      };
      remote-pdb = pkgs.python3.pkgs.callPackage ./remote-pdb.nix { };
      bitcoind = pkgs.bitcoind.override { withGui = false; };
      cockroachdb = pkgs.callPackage ./cockroachdb.nix { };
      default = self'.packages.kld;
    };
  };
}
