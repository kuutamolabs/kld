{ stdenv, lightning-knd }:
stdenv.mkDerivation {
  name = "tests";
  src = ../..;
  nativeBuildInputs = [ lightning-knd ];
  doCheck = true;
  checkPhase = ''
    cargo test --all-targets --all-features
  '';
  installPhase = "touch $out";
}
