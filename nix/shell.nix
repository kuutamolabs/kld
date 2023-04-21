{
  perSystem =
    { config
    , self'
    , pkgs
    , ...
    }:
    let
      systemInfoUpdateScript = pkgs.writeShellScriptBin "system-info" ''
        #!/usr/bin/env bash
        if [ ! -f "system-info.toml" ];
        then
            echo "Please run this script at project root"
            exit 1
        fi

        find . -type f                              \
            -a -not -path './target*'               \
            -a -not -path './.git/*'                \
            -a -not -path './result/*'              \
            -a -not -path './test-utils/*'          \
            -a -not -path './.github/*'             \
            -a -not -path './nix/modules/tests/*'   \
            -a -not -name 'system-info.toml'        \
            -a -not -name 'README.md'               \
            -exec sh -c '
              for f do
                git check-ignore -q "$f" || sha256sum "$f"
              done
            ' find-sh {} + | sort > .project-hash

        if [ "$1" == "check" ];
        then
            head -n 2 system-info.toml >> .project-hash
            calculate_hash="project_hash = \""$(sha256sum .project-hash | cut -d ' ' -f 1 | tr -d '\n')"\""
            recorded_hash=$(tail -n 1 system-info.toml)
            if [ "$calculate_hash" != "$recorded_hash" ];
            then
                cat .project-hash
                echo ""
                echo "Project hash does not match"
                echo "current:  "$calculate_hash
                echo "recorded: "$recorded_hash
                echo "Please run \`scripts/systemo-info\` under project root to update"
                exit 1
            fi
        else
            echo  -n "git_sha = \"" > system-info.toml
            git rev-parse HEAD  | tr -d '\n' >> system-info.toml
            echo "\"" >> system-info.toml

            echo  -n "git_commit_date = \"" >> system-info.toml
            git --no-pager log -1 --pretty=format:'%cs' >> system-info.toml
            echo "\"" >> system-info.toml


            cat system-info.toml >> .project-hash

            echo  -n "project_hash = \"" >> system-info.toml
            sha256sum .project-hash | cut -d ' ' -f 1 | tr -d '\n' >> system-info.toml
            echo "\"" >> system-info.toml
            cat system-info.toml
            git add system-info.toml
            git commit -m 'update system-info.toml'
        fi
      '';
    in
    {
      devShells.default = pkgs.mkShell {
        packages = [
          # code formatting
          config.treefmt.build.wrapper

          # tasks and automation
          pkgs.just
          pkgs.jq
          pkgs.nix-update

          # rust dev
          pkgs.rust-analyzer
          pkgs.cargo-watch
          pkgs.clippy

          # crane does not have this in nativeBuildInputs
          pkgs.rustc

          # dev and ci
          systemInfoUpdateScript
        ] ++ config.packages.kld.nativeBuildInputs
        ++ config.packages.kld.tests.nativeBuildInputs;
        inherit (config.packages.kld) buildInputs;
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE = 1;
        inherit (self'.packages.kld) nativeBuildInputs;
      };
    };
}
