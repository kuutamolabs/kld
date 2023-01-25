{ craneLib
, lib
, clippy
, openssl
, bitcoind
, cockroachdb
, teos
, pkg-config
, self
}:
let
  paths = [
    "Cargo.toml"
    "Cargo.lock"
    "src"
    "api"
    "bitcoind"
    "database"
    "logger"
    "settings"
    "tests"
    "test-utils"
  ];
  src = lib.cleanSourceWith {
    src = self;
    filter = path: _type: lib.any (p: lib.hasPrefix "${self}/${p}" path) paths;
  };
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config ];
  cargoExtraArgs = "--workspace --all-features";
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src buildInputs nativeBuildInputs cargoExtraArgs;
  };
in
craneLib.buildPackage {
  name = "lightning-knd";
  inherit src cargoArtifacts buildInputs nativeBuildInputs;
  cargoExtraArgs = "${cargoExtraArgs} --all-targets";
  passthru.clippy = craneLib.cargoClippy {
    inherit src cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs;
    cargoClippyExtraArgs = "--no-deps -- -D warnings";
  };

  checkInputs = [ bitcoind cockroachdb teos ];

  doCheck = true;

  meta = with lib; {
    description = "HA Bitcoin Lightning Node";
    homepage = "https://github.com/kuutamoaps/lightning-knd";
    license = licenses.asl20;
    maintainers = with maintainers; [ mic92 ];
    platforms = platforms.unix;
  };
}
