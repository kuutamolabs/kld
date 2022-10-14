{ runCommand, git, formatters, self }:
runCommand "check-format"
{
  nativeBuildInputs = formatters;
} ''
  # keep timestamps so that treefmt is able to detect mtime changes
  cp --no-preserve=mode --preserve=timestamps -r ${self} source
  cd source
  HOME=$TMPDIR treefmt --fail-on-change
  touch $out
''
