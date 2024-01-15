{ craneLib
, lib
, clippy
, openssl
, bitcoind
, cockroachdb
, electrs
, pkg-config
, sqlite
, self
}:
let
  paths = [
    "Cargo.toml"
    "Cargo.lock"
    "kld"
    "benches"
    "settings"
    "test-utils"
    "tui"
  ];
  src = lib.cleanSourceWith {
    src = self;
    filter = path: _type: lib.any (p: lib.hasPrefix "${self}/${p}" path) paths;
  };
  cargoToml = "${src}/kld/Cargo.toml";
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config sqlite ];
  cargoExtraArgs = "--workspace --all-features";
  outputHashes = {
    "git+https://github.com/kuutamolabs/bdk?branch=0.29.0-allow-begin-match-fail#d983732c7e290caff336a39981b6159b4c44c22e" = "sha256-63rvs64cGW2JzYWQQRMhIM5hILBpGjdsR8VTpjHuNPE=";
    "git+https://github.com/hyperium/mime#938484de95445a2af931515d2b7252612c575da7" = "sha256-Zdhw4wWK2ZJrv62YoJMdTHaQhIyKxtG2UCu/m3mQwy0=";
    "git+https://github.com/kuutamolabs/ldk-lsp-client?branch=kuutamo-0.0.119#5e0076ca97a30cd02402af78f914eb7de32e1a2a" = "sha256-bIRM2qS7b3eoeCtEcw0eIRK/YpidKQ3bR9ueHR8yocc=";
    "git+https://github.com/yanganto/ratatui?branch=table-footer#5268fd23cc85d8043335f74262b3fda729ca9750" = "sha256-j2rzlIZBF4mKLfIoNfBYAUjeBKYPJzJcnON6X8A7jbw=";
  };
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoToml buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
  };
in
craneLib.buildPackage {
  name = "kld";
  inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs outputHashes;
  cargoExtraArgs = "${cargoExtraArgs} --bin kld --bin kld-cli --examples --lib";

  # avoid recompiling openssl
  preBuild = ''
    rm -rf ./target
    cp -r ${cargoArtifacts} ./target
    chmod -R u+w ./target
  '';

  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
    benches = craneLib.mkCargoDerivation {
      inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
      buildPhaseCargoCommand = "cargo bench --no-run";
    };
    # having the tests separate avoids having to run them on every package change.
    tests = craneLib.cargoTest {
      inherit src cargoToml cargoArtifacts buildInputs cargoExtraArgs outputHashes;
      # FIXME: this copy shouldn't be necessary, but for some reason it tries to recompile openssl and fails
      preBuild = ''
        rm -rf ./target
        cp -r ${cargoArtifacts} ./target
        chmod -R u+w ./target
      '';
      nativeBuildInputs = nativeBuildInputs ++ [ bitcoind cockroachdb electrs ];
      FLAKE_CHECK = true;
    };
    inherit cargoArtifacts;
  };
  postInstall = ''
    find target/release/build -name kld-cli.bash -exec install -D -m755 {} $out/bin/kld-cli.bash \;
  '';
  # we run tests in a separate package
  doCheck = false;

  meta = with lib; {
    description = "Lightning Network Kuutamo Node Distribution";
    mainProgram = "kld";
    homepage = "https://github.com/kuutamolabs/kld";
    license = licenses.asl20;
    platforms = platforms.unix;
  };
}
