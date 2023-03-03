{
  perSystem = { config, ... }: {
    checks = {
      kld-clippy = config.packages.kld.clippy;
      kld-benches = config.packages.kld.benches;
      kld-tests = config.packages.kld.tests;
    };
  };
}
