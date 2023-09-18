{ self, lib, ... }:

{
  perSystem = { pkgs, self', ... }: {
    checks = lib.optionalAttrs pkgs.stdenv.isLinux {
      kld = import ./kld.nix { inherit self pkgs; };
      kld-mgr = import ./kld-mgr.nix { inherit self pkgs; };
      cockroachdb = import ./cockroachdb.nix { inherit self pkgs; };
      generated-example-is-same = pkgs.runCommand "generated-example-is-same" { } ''
        if ! diff <(${lib.getExe self'.packages.kld-mgr} generate-example) "${self}/example/kld.toml"; then
          echo "Generated example in /example is no longer up-to-date!!" >&2
          echo "run the following command:" >&2
          echo "$ cd mgr; cargo run generate-example > ../example/kld.toml" >&2
          exit 1
        fi
        touch $out
      '';
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
        deployment_flake = "dirty"
      '';
    };
  } // import ./test-flake/configurations.nix {
    lightning-knd = self;
  };
}
