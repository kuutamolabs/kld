(import ./lib.nix) {
  name = "from-nixos";
  nodes = {
    # self here is set by using specialArgs in `lib.nix`
    node1 = { self, ... }: {
      imports = [ self.nixosModules.lightning-knd ];
    };
  };
  # This is the test code that will check if our service is running correctly:
  testScript = ''
    start_all()
    # wait for our service to start
    node1.wait_for_unit("lightning-knd")
  '';
}
