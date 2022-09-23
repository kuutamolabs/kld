{ rustPlatform, fetchFromGitHub, callPackage, protobuf, rustfmt, fetchpatch }:
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

  nativeBuildInputs = [
    protobuf
    rustfmt
  ];

  postPatch = ''
    mkdir -p web-admin/build
    cp -r ${web-admin}/* web-admin/build
  '';
  # Disable bitcoind crate because it downloads a bitcoind binary at test time
  doCheck = false;

  cargoSha256 = "sha256-X66spc6TzHfGUxqvtSYHtuMrHg3iiawJaxgnRWDRkCk=";

  passthru.updateScript = callPackage ./update.nix {};
}
