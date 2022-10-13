{ rustPlatform
, lib
, clippy
, openssl
, pkg-config
, runCommand
, enableLint ? false
,
}:
rustPlatform.buildRustPackage ({
  name = "lightning-knd" + lib.optionalString enableLint "-clippy";
  # avoid trigger rebuilds if unrelated files are changed
  src = runCommand "src" { } ''
    install -D ${../../Cargo.toml} $out/Cargo.toml
    install -D ${../../Cargo.lock} $out/Cargo.lock
    cp -r ${../../src} $out/src
  '';
  cargoLock.lockFile = ../../Cargo.lock;

  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config ] ++ lib.optionals enableLint [ clippy ];

  doCheck = false;

  meta = with lib; {
    description = "HA Bitcoin Lightning Node";
    homepage = "https://github.com/kuutamoaps/kuutamocore";
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
  '';
  buildPhase = ''
    cargo clippy --all-targets --all-features -- -D warnings
    if grep -R 'dbg!' ./src; then
      echo "use of dbg macro found in code!"
      false
    fi
  '';
  installPhase = ''
    touch $out
  '';
})
