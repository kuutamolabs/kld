{ craneLib
, lib
, openssl
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
  cargoToml = "${src}/tui/Cargo.toml";
  buildInputs = [ openssl sqlite ];
  nativeBuildInputs = [ pkg-config ];
  cargoExtraArgs = "--workspace --all-features";
  outputHashes = {
    "git+https://github.com/kuutamolabs/bdk?branch=0.28.2-allow-begin-match-fail#198c698c6c97055fc3f88f8af92f42a45ef709eb" = "sha256-jDYkAS1u6yl5MgBaPUS+RIcf47vCdnBsRS9/IHeS6yI=";
    "git+https://github.com/hyperium/mime#938484de95445a2af931515d2b7252612c575da7" = "sha256-Zdhw4wWK2ZJrv62YoJMdTHaQhIyKxtG2UCu/m3mQwy0=";
    "git+https://github.com/kuutamolabs/ldk-lsp-client?branch=kuutamo#e75f1a2a1510b6ef8d565332dda0f9230bf3ef7f" = "sha256-AmA0MX35bjHx9ocqSxxcnzpxDE7fTRvfwsCfV0P2khQ=";
  };
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoToml buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
  };
in
craneLib.buildPackage {
  name = "kld";
  inherit src cargoToml cargoArtifacts buildInputs nativeBuildInputs outputHashes;
  cargoExtraArgs = "${cargoExtraArgs} --bin kld-tui --examples --lib";

  # avoid recompiling openssl
  preBuild = ''
    rm -rf ./target
    cp -r ${cargoArtifacts} ./target
    chmod -R u+w ./target
  '';

  # we run tests in a separate package
  doCheck = false;

  meta = with lib; {
    description = "Lightning Network Kuutamo Node Distribution";
    mainProgram = "kld-tui";
    homepage = "https://github.com/kuutamolabs/kld";
    license = licenses.asl20;
    platforms = platforms.unix;
  };
}
