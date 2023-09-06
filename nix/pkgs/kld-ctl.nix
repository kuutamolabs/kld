{ lib
, clippy
, self
, hostPlatform
}:
let
  paths = [ "ctl" ];
  src = lib.cleanSourceWith {
    src = self + "/ctl";
    filter = path: _type: lib.any (p: lib.hasPrefix "${self}/${p}" path) paths;
  };
  cargoExtraArgs = "--workspace --all-features";
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoExtraArgs;
  };
  craneLib = self.inputs.crane.lib.${hostPlatform.system};
in
craneLib.buildPackage {
  name = "kld-ctl";
  inherit src cargoArtifacts;
  cargoExtraArgs = "${cargoExtraArgs} --bin kld-ctl --examples";
  passthru = {
    clippy = craneLib.cargoClippy {
      inherit src cargoArtifacts cargoExtraArgs;
      cargoClippyExtraArgs = "--all-targets --no-deps -- -D warnings";
    };
  };

  # we run tests in a separate package
  doCheck = false;

  meta = with lib; {
    description = "Lightning Network Kuutamo Node Distribution";
    homepage = "https://github.com/kuutamolabs/kld";
    license = licenses.asl20;
    platforms = platforms.unix;
  };
}
