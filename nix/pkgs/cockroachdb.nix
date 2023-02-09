{ stdenv, fetchurl, autoPatchelfHook, lib }:
stdenv.mkDerivation rec {
  pname = "cockroachdb";
  version = "22.2.3";
  src = fetchurl {
    url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-amd64.tgz";
    sha256 = "sha256-CHuk4lYf6YQiOmcAcjkHuzL3PW/o55G99a5JjbKF4NY=";
  };
  buildInputs = [ stdenv.cc.cc ];
  nativeBuildInputs = [ autoPatchelfHook ];

  installPhase = ''
    install -D -m755 cockroach $out/bin/cockroach
    cp -r lib $out/lib
  '';
  meta = with lib; {
    homepage = "https://www.cockroachlabs.com";
    description = "A scalable, survivable, strongly-consistent SQL database";
    platforms = [ "x86_64-linux" ];
    maintainers = with maintainers; [ mic92 ];
  };
}
