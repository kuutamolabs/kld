{ buildPackages, nodejs, stdenv, src }:

let
  nodeComposition = import ./node-composition.nix {
    inherit (buildPackages) nodejs;
    inherit (stdenv.hostPlatform) system;
    pkgs = buildPackages;
  };
in
nodeComposition.package.override {
  name = "sensei-webadmin";
  src = "${src}/web-admin";

  dontNpmInstall = true;

  postInstall = ''
    cd $out
    mv lib/node_modules/web-admin/build/* .
    rm -rf lib
  '';
}
