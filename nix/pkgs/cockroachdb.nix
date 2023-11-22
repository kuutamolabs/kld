{ stdenv, fetchurl, autoPatchelfHook, lib }:
let
  version = "23.1.4";

  srcs = {
    "x86_64-linux" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-amd64.tgz";
      sha256 = "sha256-MSX4U4nIG9TUQ8tugmzL3Y60pJ7y05f5W9suO9bnmss=";
    };
  };
in
stdenv.mkDerivation {
  pname = "cockroachdb";
  inherit version;
  src = fetchurl srcs.${stdenv.system};
  buildInputs = [ stdenv.cc.cc ];
  nativeBuildInputs = lib.optional stdenv.isLinux [ autoPatchelfHook ];

  installPhase = ''
    install -D -m755 cockroach $out/bin/cockroach
    if [[ -d lib ]]; then
      cp -r lib $out/lib
    fi
  '';
  meta = with lib; {
    homepage = "https://www.cockroachlabs.com";
    description = "A scalable, survivable, strongly-consistent SQL database";
    platforms = [
      "x86_64-linux"
    ];
    maintainers = with maintainers; [ mic92 ];
  };
}
