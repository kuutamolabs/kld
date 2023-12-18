{ self, ... }: {
  perSystem = { config, self', pkgs, ... }: rec {
    packages = {
      kld = pkgs.callPackage ./kld.nix {
        inherit self;
        craneLib = pkgs.nightlyCraneLib;
        inherit (config.packages) cockroachdb;
      };
      kld-mgr = pkgs.callPackage ./kld-mgr.nix {
        inherit self;
      };
      kld-tui = pkgs.callPackage ./kld-tui.nix {
        craneLib = pkgs.nightlyCraneLib;
        inherit self;
      };
      kld-ctl = pkgs.callPackage ./kld-ctl.nix {
        inherit self;
      };
      kld-cli = pkgs.writeScriptBin "kld-cli" ''
        ${packages.kld}/bin/kld-cli "$@"
      '';
      remote-pdb = pkgs.python3.pkgs.callPackage ./remote-pdb.nix { };
      bitcoind = pkgs.bitcoind.override { withGui = false; };
      inherit (pkgs) electrs;
      cockroachdb = pkgs.callPackage ./cockroachdb.nix { };
      default = self'.packages.kld-mgr;
    };
  };
}
