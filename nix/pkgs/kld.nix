{ craneLib
, lib
, clippy
, openssl
, bitcoind
, cockroachdb
, electrs
, pkg-config
, rsync
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
    "https://github.com/JosephGoulden/bdk?branch=backport-begin-batch-result" = "sha256-Z48LIgN8/qfgGvzjPQnn39xK3nVsCWF9uIm0xwCTDhA=";
    "https://github.com/hyperium/mime" = "sha256-Zdhw4wWK2ZJrv62YoJMdTHaQhIyKxtG2UCu/m3mQwy0=";
  };
  # this is a bit of an hack, since we have to copy the vendor dir and find the broken symlink and replace it with the real file
  # we should remove (or disable) this if pointing to a stable release again
  cargoVendorDir = (craneLib.vendorCargoDeps { inherit src; }).overrideAttrs (old: {
    buildCommand = ''
      env >&2
      ${old.buildCommand}
      mv "$out" broken
      ${rsync}/bin/rsync -a --copy-unsafe-links --chmod=u=rwX broken/ "$out/"
      src=$(find "$out" -type f -wholename '*util/time.rs')
      dst=$(find "$out" -type l -name 'time_utils.rs')
      rm -f "$dst"
      cp -f "$src" "$dst"
    '';
  });
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoToml buildInputs nativeBuildInputs cargoExtraArgs outputHashes cargoVendorDir;
  };
in
craneLib.buildPackage {
  name = "kld";
  inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs outputHashes cargoVendorDir;
  cargoExtraArgs = "${cargoExtraArgs} --bins --examples --lib";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes cargoVendorDir;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
    benches = craneLib.mkCargoDerivation {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes cargoVendorDir;
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
    inherit cargoArtifacts cargoVendorDir;
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
