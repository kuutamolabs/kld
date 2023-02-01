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
    "benches"
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
  cargoExtraArgs = "${cargoExtraArgs} --bins --examples --lib";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
    benches = craneLib.mkCargoDerivation {
      inherit src cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs;
      buildPhaseCargoCommand = "cargo bench --no-run";
    };
    # having the tests seperate avoids having to run them on every package change.
    tests = craneLib.cargoTest {
      inherit src cargoArtifacts buildInputs cargoExtraArgs;
      nativeBuildInputs = nativeBuildInputs ++ [ bitcoind cockroachdb teos ];
    };
  };

  # we run tests in a seperate package
  doCheck = false;

  meta = with lib; {
    description = "HA Bitcoin Lightning Node";
    homepage = "https://github.com/kuutamoaps/lightning-knd";
    license = licenses.asl20;
    maintainers = with maintainers; [ mic92 ];
    platforms = platforms.unix;
  };
}
