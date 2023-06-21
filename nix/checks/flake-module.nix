{
  perSystem = { config, ... }: {
    checks = {
      kld-clippy = config.packages.kld.clippy;
      kld-benches = config.packages.kld.benches;
      #kld-tests = config.packages.kld.tests;

      kld-mgr-clippy = config.packages.kld-mgr.clippy;
      kld-mgr-tests = config.packages.kld-mgr.tests;
    };
  };
}
