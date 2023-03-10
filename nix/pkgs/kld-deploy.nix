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
  paths = [ "deploy" ];
  src = lib.cleanSourceWith {
    src = self + "/deploy";
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
  name = "kld";
  inherit src cargoArtifacts buildInputs nativeBuildInputs;
  cargoExtraArgs = "${cargoExtraArgs} --bins --examples --lib";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
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
    description = "Lightning Network Kuutamo Node Distribution";
    homepage = "https://github.com/kuutamolabs/kld";
    license = licenses.asl20;
    platforms = platforms.unix;
  };
}
