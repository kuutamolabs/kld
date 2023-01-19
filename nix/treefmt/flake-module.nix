{ inputs, ... }: {
  imports = [
    inputs.treefmt-nix.flakeModule
  ];

  perSystem =
    { pkgs
    , lib
    , ...
    }: {
      treefmt = {
        # Used to find the project root
        projectRootFile = "flake.lock";

        programs.rustfmt.enable = true;

        settings.formatter = {
          nix = {
            command = pkgs.runtimeShell;
            options = [
              "-eucx"
              ''
                export PATH=${lib.makeBinPath [pkgs.coreutils pkgs.findutils pkgs.statix pkgs.deadnix pkgs.nixpkgs-fmt]}
                deadnix --edit "$@"
                # statix breaks flake.nix's requirement for making outputs a function
                echo "$@" | xargs -P$(nproc) -n1 statix fix -i flake.nix node-env.nix
                nixpkgs-fmt "$@"
              ''
              "--"
            ];
            includes = [ "*.nix" ];
          };

          shell = {
            command = pkgs.runtimeShell;
            options = [
              "-eucx"
              ''
                ${pkgs.lib.getExe pkgs.shellcheck} --external-sources --source-path=SCRIPTDIR "$@"
                ${pkgs.lib.getExe pkgs.shfmt} -i 2 -s -w "$@"
              ''
              "--"
            ];
            includes = [ "*.sh" ];
          };
        };
      };
    };
}
