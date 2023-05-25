{ self, lib, ... }:

{
  perSystem = { pkgs, ... }: {
    checks = lib.optionalAttrs pkgs.stdenv.isLinux {
      kld = import ./kld.nix { inherit self pkgs; };
      kld-mgr = import ./kld-mgr.nix { inherit self pkgs; };
      cockroachdb = import ./cockroachdb.nix { inherit self pkgs; };
    };
  };

  flake = {
    nixosModules.qemu-test-profile = { modulesPath, ... }: {
      imports = [
        (modulesPath + "/testing/test-instrumentation.nix")
        (modulesPath + "/profiles/qemu-guest.nix")
      ];
      environment.etc."system-info.toml".text = ''
        git_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        git_commit_date = "20230424000000"
      '';
    };
  } // import ./test-flake/configurations.nix {
    lightning-knd = self;
  };
}
