{ craneLib
, lib
, clippy
, openssl
, bitcoind
, cockroachdb
, electrs
, pkg-config
, self
}:
let
  paths = [
    "Cargo.toml"
    "Cargo.lock"
    "api"
    "kld"
    "benches"
    "settings"
    "test-utils"
  ];
  src = lib.cleanSourceWith {
    src = self;
    filter = path: _type: lib.any (p: lib.hasPrefix "${self}/${p}" path) paths;
  };
  cargoToml = "${src}/kld/Cargo.toml";
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config ];
  cargoExtraArgs = "--workspace --all-features";
  outputHashes = {
    "git+https://github.com/JosephGoulden/bdk?branch=backport-begin-batch-result#39d8626e8c40455b6089975fda79000941094910" = "sha256-Z48LIgN8/qfgGvzjPQnn39xK3nVsCWF9uIm0xwCTDhA=";
    "git+https://github.com/hyperium/mime#938484de95445a2af931515d2b7252612c575da7" = "sha256-Zdhw4wWK2ZJrv62YoJMdTHaQhIyKxtG2UCu/m3mQwy0=";
    "https://github.com/lightningdevkit/ldk-lsp-client" = "sha256-vtDX0+9YSgOYndxM+R58mn/AI34rt0RAwwavCCKl3Lw="; # 2fa0478
  };
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoToml buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
  };
in
craneLib.buildPackage {
  name = "kld";
  inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs outputHashes;
  cargoExtraArgs = "${cargoExtraArgs} --bins --examples --lib";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
    benches = craneLib.mkCargoDerivation {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
      buildPhaseCargoCommand = "cargo bench --no-run";
    };
    # having the tests seperate avoids having to run them on every package change.
    tests = craneLib.cargoTest {
      inherit src cargoToml cargoArtifacts buildInputs cargoExtraArgs outputHashes;
      # FIXME: this copy shouldn't be necessary, but for some reason it tries to recompile openssl and fails
      preBuild = ''
        rm -rf ./target
        cp -r ${cargoArtifacts} ./target
        chmod -R u+w ./target
      '';
      nativeBuildInputs = nativeBuildInputs ++ [ bitcoind cockroachdb electrs ];
    };
    inherit cargoArtifacts;
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
