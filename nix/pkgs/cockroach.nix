{ lib
, stdenv
, buildGoModule
, fetchurl
, cmake
, xz
, which
, autoconf
, ncurses6
, libedit
, libunwind
, installShellFiles
, ps
, removeReferencesTo
, go
}:

let
  darwinDeps = [ libunwind libedit ];
  linuxDeps = [ ncurses6 ];

  buildInputs = if stdenv.isDarwin then darwinDeps else linuxDeps;
  nativeBuildInputs = [ installShellFiles cmake xz which autoconf ];
in
buildGoModule rec {
  pname = "cockroach";
  version = "21.1.21";

  src = fetchurl {
    url = "https://binaries.cockroachdb.com/cockroach-v${version}.src.tgz";
    hash = "sha256-qz2A+oYcwxVDh23AnDuwDY05n57zwe59GjDjwJ4zX6U=";
  };
  vendorSha256 = null;

  NIX_CFLAGS_COMPILE = lib.optionals stdenv.cc.isGNU [ "-Wno-error=deprecated-copy" "-Wno-error=redundant-move" "-Wno-error=pessimizing-move" ];

  inherit nativeBuildInputs buildInputs;

  postPatch = ''
    patchShebangs .
  '';

  buildPhase = ''
    runHook preBuild
    make buildoss
    cd src/github.com/cockroachdb/cockroach
    for asset in man autocomplete; do
      ./cockroachoss gen $asset
    done
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    install -D cockroachoss $out/bin/cockroach
    installShellCompletion cockroach.bash

    installManPage man/man1/*

    runHook postInstall
  '';

  outputs = [ "out" "man" ];

  # fails with `GOFLAGS=-trimpath`
  allowGoReference = true;
  preFixup = ''
    find $out -type f -exec ${removeReferencesTo}/bin/remove-references-to -t ${go} '{}' +
  '';

  meta = with lib; {
    homepage = "https://www.cockroachlabs.com";
    description = "A scalable, survivable, strongly-consistent SQL database";
    license = licenses.bsl11;
    platforms = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" ];
    maintainers = with maintainers; [ rushmorem thoughtpolice ];
  };
}
