{
  description = "Bitcoin Lightning node";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  nixConfig.extra-substituters = [
    "https://cache.garnix.io"
  ];
  nixConfig.extra-trusted-public-keys = [
    "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
  ];

  outputs = inputs @ { flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./nix/pkgs/flake-module.nix
        ./nix/modules/flake-module.nix
        ./nix/modules/tests/flake-module.nix
        ./nix/checks/flake-module.nix
        ./nix/treefmt/flake-module.nix
        ./nix/shell.nix
      ];
      systems = [ "x86_64-linux" ];

      perSystem = { system, ... }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          config.allowUnfreePredicate = (pkg: builtins.elem
            (builtins.parseDrvName pkg.pname).name [
            "cockroach"
          ]);
        };
      };
    };
}
