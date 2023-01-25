(import ./lib.nix) {
  name = "from-nixos";
  nodes = {
    # self here is set by using specialArgs in `lib.nix`
    node1 = { self, ... }: {
      imports = [ self.nixosModules.lightning-knd ];
    };
  };
  # This test is still wip
  testScript = ''
    start_all()

    # wait for our service to start
    node1.wait_for_unit("lightning-knd")
    # FIXME: we still need to configure bitcoind so that the service can start correctly
    #node1.wait_until_succeeds("curl -v http://127.0.0.1:2233/metrics >&2")
  '';
}
