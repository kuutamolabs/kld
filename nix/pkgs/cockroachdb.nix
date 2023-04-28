{ stdenv, fetchurl, autoPatchelfHook, lib }:
let
  version = "22.2.3";
  srcs = {
    "x86_64-linux" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-amd64.tgz";
      sha256 = "sha256-CHuk4lYf6YQiOmcAcjkHuzL3PW/o55G99a5JjbKF4NY=";
    };
    "aarch64-darwin" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.darwin-11.0-arm64.tgz";
      sha256 = "sha256-Zu85JhQ4VvblNSXJMuY1yE2AOBTrv9uiIrwtevqj2RA=";
    };
  };
in
stdenv.mkDerivation rec {
  pname = "cockroachdb";
  inherit version;
  src = fetchurl srcs.${stdenv.system};
  buildInputs = [ stdenv.cc.cc ];
  nativeBuildInputs = lib.optional stdenv.isLinux [ autoPatchelfHook ];

  installPhase = ''
    install -D -m755 cockroach $out/bin/cockroach
    cp -r lib $out/lib
  '';
  meta = with lib; {
    homepage = "https://www.cockroachlabs.com";
    description = "A scalable, survivable, strongly-consistent SQL database";
    platforms = [
      "aarch64-darwin"
      "x86_64-linux"
    ];
    maintainers = with maintainers; [ mic92 ];
  };
}
