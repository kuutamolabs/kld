{ rustPlatform
, lib
, clippy
, openssl
, bitcoind
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
    cp -r ${../../tests} $out/tests
    cp -r ${../../test-utils} $out/test-utils
  '';
  cargoLock.lockFile = ../../Cargo.lock;

  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config bitcoind ] ++ lib.optionals enableLint [ clippy ];

  doCheck = enableTests;

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
    cp -r ${../../tests} $out/tests
    cp -r ${../../test-utils} $out/test-utils
  '';
  buildPhase = ''
    cargo clippy --all-targets --all-features --no-deps -- -D warnings
    if grep -R 'dbg!' ./src; then
      echo "use of dbg macro found in code!"
      false
    fi
  '';
  installPhase = ''
    touch $out
  '';
})
