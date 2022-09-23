{ rustPlatform, fetchFromGitHub, callPackage }:
let
  src = fetchFromGitHub {
    owner = "L2-Technology";
    repo = "sensei";
    rev = "bb197d0a81533e17f1f3be1f020c8acc218436ae";
    sha256 = "sha256-Do3Wq3IYj6tSBieo09rS2c7qG+wCFSQ5k2bn362MNyk=";
  };
  web-admin = callPackage ./web-admin {
    inherit src;
  };
in rustPlatform.buildRustPackage {
  inherit src;
  name = "sensei";

  postPatch = ''
    cp -r ${web-admin}/* web-admin/build
  '';

  cargoSha256 = "sha256-X66spc6TzHfGUxqvtSYHtuMrHg3iiawJaxgnRWDRkCk=";

  passthru.updateScript = callPackage ./update.nix {};
}
