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

  cargoPatches = [
    # https://github.com/L2-Technology/sensei/pull/115
    (fetchpatch {
      url = "https://github.com/L2-Technology/sensei/commit/82ce4440a00ed43b0c81b084403ae949bb3ee3b5.patch";
      sha256 = "sha256-vwO1LgzmM1oRc283XX2QQ/9zudAZNsf/TNqZE8j5O+Y=";
    })
  ];

  postPatch = ''
    mkdir -p web-admin/build
    cp -r ${web-admin}/* web-admin/build
  '';
  doCheck = false;

  cargoSha256 = "sha256-bSkTC2jL3KGxwukbHmfSBHJxwJtlMrjrqNpOY4d6x/M=";

  passthru.updateScript = callPackage ./update.nix {};
}
