{ lib
, clippy
, openssl
, pkg-config
, self
, nix
, nixos-rebuild
, rsync
, git
, openssh
, hostPlatform
, makeWrapper
}:
let
  paths = [ "mgr" ];
  src = lib.cleanSourceWith {
    src = self + "/mgr";
    filter = path: _type: lib.any (p: lib.hasPrefix "${self}/${p}" path) paths;
  };
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config makeWrapper ];
  checkInputs = [ nix ];

  cargoExtraArgs = "--workspace --all-features";
  outputHashes = {
    "https://github.com/AlexanderThaller/format_serde_error" = "sha256-R4zD1dAfB8OmlfYUDsDjevMkjfIWGtwLRRYGGRvZ8F4=";
  };
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
  };
  craneLib = self.inputs.crane.lib.${hostPlatform.system};
in
craneLib.buildPackage {
  name = "kld-mgr";
  inherit src cargoArtifacts buildInputs nativeBuildInputs outputHashes;
  cargoExtraArgs = "${cargoExtraArgs} --bins --examples --lib";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoArtifacts buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
    # having the tests seperate avoids having to run them on every package change.
    tests = craneLib.cargoTest {
      inherit src cargoArtifacts buildInputs cargoExtraArgs outputHashes;
      nativeBuildInputs = nativeBuildInputs ++ checkInputs;
    };
  };

  # openssh is suffixed so we use the host's openssh to avoid this
  # https://github.com/numtide/nixos-anywhere/issues/62 from happening
  postInstall = ''
    wrapProgram $out/bin/kld-mgr \
      --prefix PATH : ${lib.makeBinPath [
          self.inputs.nixos-anywhere.packages.${hostPlatform.system}.nixos-anywhere
          self.packages.${hostPlatform.system}.cockroachdb

          nixos-rebuild nix git rsync openssl
      ]} \
      --suffix PATH : ${lib.makeBinPath [ openssh ]}
  '';

  # we run tests in a seperate package
  doCheck = false;

  meta = with lib; {
    description = "Lightning Network Kuutamo Node Distribution";
    homepage = "https://github.com/kuutamolabs/kld";
    license = licenses.asl20;
    platforms = platforms.unix;
    mainProgram = "kld-mgr";
  };
}
