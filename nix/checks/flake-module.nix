{
  perSystem = { config, ... }: {
    checks = {
      lightning-knd-clippy = config.packages.lightning-knd.clippy;
      lightning-knd-benches = config.packages.lightning-knd.benches;
    };
  };
}
