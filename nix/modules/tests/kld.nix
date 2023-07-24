(import ./lib.nix) ({ self, pkgs, ... }: {
  name = "kld";
  nodes = {
    # self here is set by using specialArgs in `lib.nix`
    db1 = { self, ... }: {
      imports = [
        self.nixosModules.kld
        self.nixosModules.electrs
        self.nixosModules.telegraf
      ];
      # use the same name as the cert
      kuutamo.cockroachdb.nodeName = "db1";
      virtualisation.cores = 4;
      virtualisation.memorySize = 5120;

      kuutamo.cockroachdb.caCertPath = ./cockroach-certs/ca.crt;
      kuutamo.cockroachdb.nodeCertPath = ./cockroach-certs + "/db1.crt";
      kuutamo.cockroachdb.nodeKeyPath = ./cockroach-certs + "/db1.key";
      kuutamo.cockroachdb.rootClientCertPath = ./cockroach-certs + "/client.root.crt";
      kuutamo.cockroachdb.rootClientKeyPath = ./cockroach-certs + "/client.root.key";

      kuutamo.kld.cockroachdb.clientCertPath = ./cockroach-certs + "/client.kld.crt";
      kuutamo.kld.cockroachdb.clientKeyPath = ./cockroach-certs + "/client.kld.key";

      kuutamo.kld.caPath = ./kld-certs/ca.pem;
      kuutamo.kld.certPath = ./kld-certs/kld.pem;
      kuutamo.kld.keyPath = ./kld-certs/kld.key;
      kuutamo.kld.network = "regtest";
      kuutamo.kld.mnemonicPath = ./secrets/mnemonic;
      kuutamo.kld.presetMnemonic = true;

      kuutamo.electrs.network = "regtest";

      kuutamo.telegraf = {
        configHash = "";
        hasMonitoring = false;
      };

      # IO on garnix is really slow
      virtualisation.fileSystems."/var/lib/cockroachdb" = {
        fsType = "tmpfs";
      };

    };
  };

  extraPythonPackages = _p: [ self.packages.${pkgs.system}.remote-pdb ];

  # This test is still wip
  testScript = ''
    start_all()

    # wait for our service to start
    db1.wait_for_unit("cockroachdb.service")
    db1.wait_for_unit("bitcoind-kld-regtest.service")
    db1.wait_for_unit("electrs.service")
    db1.wait_for_unit("kld.service")

    # check monitoring endpoints
    db1.wait_until_succeeds("curl -s -k https://127.0.0.1:8080/_status/vars")
    db1.wait_until_succeeds("curl -s http://127.0.0.1:4224")
    db1.wait_until_succeeds("curl -s http://127.0.0.1:2233/metrics")
    db1.wait_for_unit("telegraf.service")
    db1.wait_until_succeeds("curl -s http://127.0.0.1:9273/metrics")

    db1.wait_until_succeeds("kld-cli get-info")

    # test if we can interact with the bitcoin node
    db1.succeed("kld-bitcoin-cli createwallet testwallet >&2")
    db1.succeed("kld-bitcoin-cli -rpcwallet=testwallet -generate 6 1000")

    # useful for debugging
    def remote_shell(machine):
        machine.shell_interact("tcp:127.0.0.1:4444,forever,interval=2")

    #remote_shell(machine)
    #breakpoint()
  '';
})
