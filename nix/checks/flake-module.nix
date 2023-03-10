{
  perSystem = { config, ... }: {
    checks = {
      kld-clippy = config.packages.kld.clippy;
      kld-benches = config.packages.kld.benches;
      kld-tests = config.packages.kld.tests;

      kld-deploy-clippy = config.packages.kld-deploy.clippy;
      kld-deploy-tests = config.packages.kld-deploy.tests;
    };
  };
}
