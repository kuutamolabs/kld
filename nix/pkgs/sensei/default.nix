{ rustPlatform, fetchFromGitHub, callPackage, protobuf, rustfmt, fetchpatch }:
let
  src = fetchFromGitHub {
    owner = "L2-Technology";
    repo = "sensei";
    rev = "2c902f987628aa6ea3c1c1cbc57804dc68510b6d";
    sha256 = "sha256-HlkTTm62SCTg1tasiwxI1qSqwzKVjbQheQ4fZa5XCqI=";
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
