{ rustPlatform
, lib
, clippy
, openssl
, bitcoind
, cockroachdb
, teos
, pkg-config
, runCommand
, enableLint ? false
, enableTests ? false
,
}:
rustPlatform.buildRustPackage ({
  name = "lightning-knd" + lib.optionalString enableLint "-clippy";
  # avoid trigger rebuilds if unrelated files are changed
  src = runCommand "src" { } ''
    install -D ${../../Cargo.toml} $out/Cargo.toml
    install -D ${../../Cargo.lock} $out/Cargo.lock
    cp -r ${../../src} $out/src
    cp -r ${../../api} $out/api
    cp -r ${../../bitcoind} $out/bitcoind
    cp -r ${../../database} $out/database
    cp -r ${../../logger} $out/logger
    cp -r ${../../settings} $out/settings
    cp -r ${../../tests} $out/tests
    cp -r ${../../test-utils} $out/test-utils
  '';
  cargoLock.lockFile = ../../Cargo.lock;

  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config bitcoind cockroachdb teos ] ++ lib.optionals enableLint [ clippy ];

  doCheck = enableTests;
  checkFlags = [
    "--workspace"
    "--all-features"
    "--all-targets"
  ];
  meta = with lib; {
    description = "HA Bitcoin Lightning Node";
    homepage = "https://github.com/kuutamoaps/lightning-knd";
    license = licenses.asl20;
    maintainers = with maintainers; [ mic92 ];
    platforms = platforms.unix;
  };
}
  // lib.optionalAttrs enableLint {
  src = runCommand "src" { } ''
    install -D ${../../Cargo.toml} $out/Cargo.toml
    install -D ${../../Cargo.lock} $out/Cargo.lock
    cp -r ${../../src} $out/src
    cp -r ${../../api} $out/api
    cp -r ${../../bitcoind} $out/bitcoind
    cp -r ${../../database} $out/database
    cp -r ${../../logger} $out/logger
    cp -r ${../../settings} $out/settings
    cp -r ${../../tests} $out/tests
    cp -r ${../../test-utils} $out/test-utils
  '';
  buildPhase = ''
    cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings
    if grep -R 'dbg!' ./src; then
      echo "use of dbg macro found in code!"
      false
    fi
  '';
  installPhase = ''
    touch $out
  '';
})
