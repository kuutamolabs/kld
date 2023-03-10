{ lightning-knd, ... }: {
  nixosConfigurations."kld-00" = lightning-knd.inputs.nixpkgs.lib.nixosSystem {
    system = "x86_64-linux";
    modules = [
      lightning-knd.nixosModules."kld-node"
      lightning-knd.nixosModules."qemu-test-profile"
      { kuutamo.deployConfig = builtins.fromTOML (builtins.readFile (builtins.path { name = "node.toml"; path = ./kld-00.toml; })); }
    ];
  };
  nixosConfigurations."db-00" = lightning-knd.inputs.nixpkgs.lib.nixosSystem {
    system = "x86_64-linux";
    modules = [
      lightning-knd.nixosModules."cockroachdb-node"
      lightning-knd.nixosModules."qemu-test-profile"
      { kuutamo.deployConfig = builtins.fromTOML (builtins.readFile (builtins.path { name = "node.toml"; path = ./db-00.toml; })); }
    ];
  };
  nixosConfigurations."db-01" = lightning-knd.inputs.nixpkgs.lib.nixosSystem {
    system = "x86_64-linux";
    modules = [
      lightning-knd.nixosModules."cockroachdb-node"
      lightning-knd.nixosModules."qemu-test-profile"
      { kuutamo.deployConfig = builtins.fromTOML (builtins.readFile (builtins.path { name = "node.toml"; path = ./db-01.toml; })); }
    ];
  };
}
