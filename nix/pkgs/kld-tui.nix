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
    "git+https://github.com/kuutamolabs/ldk-lsp-client?branch=fix-cyc-dep#2a9c244643969a450a71a70fc918b1124f1c6bd1" = "sha256-2VmHHxgCK3Bcz+Nb9+j2hxoetgWff32ZuWWU6rd8PFw=";
    "git+https://github.com/yanganto/ratatui?branch=table-footer#5268fd23cc85d8043335f74262b3fda729ca9750" = "sha256-j2rzlIZBF4mKLfIoNfBYAUjeBKYPJzJcnON6X8A7jbw=";
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
