{ stdenv, fetchurl, autoPatchelfHook, lib }:
let
  version = "23.1.4";

  srcs = {
    "x86_64-linux" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-amd64.tgz";
      sha256 = "sha256-MSX4U4nIG9TUQ8tugmzL3Y60pJ7y05f5W9suO9bnmss=";
    };
    "aarch64-darwin" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.darwin-11.0-arm64.tgz";
      sha256 = "sha256-0KE20Vn7phqnuQujetdXs/AWmWMC9sjVWyzT46pg9IE=";
    };
    "x86_64-darwin" = {
      url = "https://binaries.cockroachdb.com/cockroach-v${version}.darwin-10.9-amd64.tgz";
      sha256 = "0i48bk4pdfxz7khafsl2wz3q3akrsr28r5gs2rgrivyv2bdm5501";
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
      "aarch64-darwin"
      "x86_64-darwin"
      "x86_64-linux"
    ];
    maintainers = with maintainers; [ mic92 ];
  };
}
