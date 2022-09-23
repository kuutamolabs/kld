{ writeScript
, lib
, coreutils
, runtimeShell
, git
, nix-update
, node2nix
, nix
}:

writeScript "update-sensei" ''
  #!${runtimeShell}
  PATH=${lib.makeBinPath [
      coreutils
      git
      node2nix
      nix
    ]}

  set -euo pipefail

  src=$(nix build --print-out-paths '.#sensei.src')
  adminDir=$(realpath nix/pkgs/sensei/web-admin)

  tempDir=$(mktemp -d)
  trap 'rm -rf -- "tempDir"' EXIT

  cp "$src"/web-admin/{package.json,package-lock.json} "$tempDir"
  cd "$tempDir"
  node2nix -l package-lock.json -c node-composition.nix
  cp *.nix "$adminDir"
''
