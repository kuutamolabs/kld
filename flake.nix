{
  description = "A Lightning Network Router node distribution by kuutamo ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";


    # These flakes are only used by crane at the moment, we pin them here so
    # that flake users can override them as needed.
    flake-utils.url = "github:numtide/flake-utils";
    systems.url = "github:nix-systems/default";
    flake-utils.inputs.systems.follows = "systems";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";

    # allow offline evaluation for git dependencies, see https://github.com/ipetkov/crane/pull/266
    #crane.url = "github:ipetkov/crane";
    crane.url = "github:Mic92/crane/allow-output-hashes";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    crane.inputs.flake-utils.follows = "flake-utils";
    crane.inputs.rust-overlay.follows = "rust-overlay";

    treefmt-nix.url = "github:numtide/treefmt-nix";

    ## dependencies for deploying nodes
    srvos.url = "github:numtide/srvos";
    srvos.inputs.nixpkgs.follows = "nixpkgs";

    disko.url = "github:nix-community/disko";
    disko.inputs.nixpkgs.follows = "nixpkgs";

    nixos-images.url = "github:nix-community/nixos-images";

    nixos-anywhere.url = "github:numtide/nixos-anywhere";
    nixos-anywhere.inputs.nixpkgs.follows = "nixpkgs";
    nixos-anywhere.inputs.disko.follows = "disko";
    nixos-anywhere.inputs.nixos-images.follows = "nixos-images";
    nixos-anywhere.inputs.treefmt-nix.follows = "treefmt-nix";
    nixos-anywhere.inputs.flake-parts.follows = "flake-parts";
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
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
      ];
    };
}
