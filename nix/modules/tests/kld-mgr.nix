import ./lib.nix ({ self, pkgs, lib, ... }:
let
  inherit (self.packages.x86_64-linux) kld-mgr;

  kexec-installer = self.inputs.nixos-images.packages.${pkgs.system}.kexec-installer-nixos-unstable;

  validator-system = self.nixosConfigurations.kld-00;

  dependencies = [
    validator-system.config.system.build.toplevel
    validator-system.config.system.build.diskoScript
  ] ++ builtins.map (i: i.outPath) (builtins.attrValues self.inputs);

  closureInfo = pkgs.closureInfo { rootPaths = dependencies; };

  shared = {
    virtualisation.vlans = [ 1 ];
    systemd.network = {
      enable = true;

      networks."10-eth1" = {
        matchConfig.Name = "eth1";
        linkConfig.RequiredForOnline = "no";
      };
    };
    documentation.enable = false;

    # do not try to fetch stuff from the internet
    nix.settings = {
      substituters = lib.mkForce [ ];
      hashed-mirrors = null;
      connect-timeout = 1;
      experimental-features = [ "flakes" ];
      flake-registry = pkgs.writeText "flake-registry" ''{"flakes":[],"version":2}'';
    };

    environment.etc."install-closure".source = "${closureInfo}/store-paths";
    system.extraDependencies = dependencies;
  };
  qemu-common = import (pkgs.path + "/nixos/lib/qemu-common.nix") {
    inherit lib pkgs;
  };
  interfacesNumbered = config: lib.zipLists config.virtualisation.vlans (lib.range 1 255);
  getNicFlags = config: lib.flip lib.concatMap
    (interfacesNumbered config)
    ({ fst, snd }: qemu-common.qemuNICFlags snd fst config.virtualisation.test.nodeNumber);
in
{
  name = "kld-mgr";
  nodes = {
    installer = { pkgs, ... }: {
      imports = [ shared ];
      systemd.network.networks."10-eth1".networkConfig.Address = "192.168.42.1/24";
      environment.systemPackages = [ pkgs.git ];

      system.activationScripts.rsa-key = ''
        ${pkgs.coreutils}/bin/install -D -m600 ${./ssh-keys/ssh} /root/.ssh/id_rsa
      '';
    };
    installed = {
      imports = [ shared ];
      systemd.network.networks."10-eth1".networkConfig.Address = "192.168.42.2/24";

      virtualisation.emptyDiskImages = [ 4096 4096 ];
      virtualisation.memorySize = 4096;
      networking.nameservers = [ "127.0.0.1" ];
      services.openssh.enable = true;
      services.openssh.settings.UseDns = false;
      users.users.root.openssh.authorizedKeys.keyFiles = [ ./ssh-keys/ssh.pub ];
    };
  };
  testScript = { nodes, ... }:
    ''
      def create_test_machine(oldmachine=None, args={}): # taken from <nixpkgs/nixos/tests/installer.nix>
          machine = create_machine({
            "qemuFlags":
              '-cpu max -m 4024 -virtfs local,path=/nix/store,security_model=none,mount_tag=nix-store,'
              f' -drive file={oldmachine.state_dir}/empty0.qcow2,id=drive1,if=none,index=1,werror=report'
              ' -device virtio-blk-pci,drive=drive1'
              f' -drive file={oldmachine.state_dir}/empty1.qcow2,id=drive2,if=none,index=2,werror=report'
              ' -device virtio-blk-pci,drive=drive2'
              ' ${toString (getNicFlags nodes.installed)}'
          } | args)
          driver.machines.append(machine)
          return machine

      start_all()
      installed.wait_for_unit("sshd.service")
      installed.succeed("ip -c a >&2; ip -c r >&2")

      installer.wait_for_unit("network.target")
      installer.succeed("ping -c1 192.168.42.2")
      # our test config will read from here
      installer.succeed("cp -r ${self} /root/lightning-knd")
      installer.succeed("install ${./test-config.toml} /root/test-config.toml")

      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml generate-config /tmp/config")
      installer.succeed("nixos-rebuild dry-build --flake /tmp/config#kld-00 >&2")

      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml --yes install --hosts kld-00 --debug --no-reboot --kexec-url ${kexec-installer}/nixos-kexec-installer-${pkgs.stdenv.hostPlatform.system}.tar.gz >&2")
      installer.succeed("ssh -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@192.168.42.2 -- reboot >&2")

      installed.shutdown()

      new_machine = create_test_machine(oldmachine=installed, args={ "name": "after_install" })
      new_machine.start()
      hostname = new_machine.succeed("hostname").strip()
      assert "kld-00" == hostname, f"'kld-00' != '{hostname}'"
      new_machine.succeed("cat /etc/systemd/system/kld.service | grep -q 'kld-00-alias' || (echo node alias does not set && exit 1)")

      installer.wait_until_succeeds("ssh -o StrictHostKeyChecking=no root@192.168.42.2 -- exit 0 >&2")

      new_machine.wait_for_unit("sshd.service")

      system_info = installer.succeed("${lib.getExe kld-mgr} --config  /root/test-config.toml system-info --hosts kld-00").strip()
      for version_field in ("kld-mgr version", "kld-ctl version", "git sha", "git commit date", "bitcoind version", "cockroach version", "kld-cli version"):
          assert version_field  in system_info, f"{version_field} in system info:\n{system_info}"

      system_info = installer.succeed("${lib.getExe kld-mgr} --config  /root/test-config.toml system-info --hosts db-00").strip()
      for version_field in ("kld-mgr version", "kld-ctl version", "git sha", "git commit date", "cockroach version"):
          assert version_field  in system_info, f"{version_field} not in system info:\n{system_info}"
      # TODO test actual service here

      # check tls certificates
      certs = [
        "/var/lib/secrets/cockroachdb/ca.crt",
        "/var/lib/secrets/cockroachdb/client.root.crt",
        "/var/lib/secrets/cockroachdb/client.root.key",
        "/var/lib/secrets/cockroachdb/node.crt",
        "/var/lib/secrets/cockroachdb/node.key",
        "/var/lib/secrets/kld/ca.pem",
        "/var/lib/secrets/kld/kld.pem",
        "/var/lib/secrets/kld/kld.key",
        "/var/lib/secrets/kld/client.kld.crt",
        "/var/lib/secrets/kld/client.kld.key",
      ]
      for cert in certs:
          new_machine.succeed(f"test -f {cert} || (echo {cert} does not exist >&2 && exit 1)")
          new_machine.succeed(f"test $(stat -c %a {cert}) == 600 || (echo {cert} has wrong permissions >&2 && exit 1)")
          new_machine.succeed(f"test $(stat -c %U {cert}) == root || (echo {cert} does not belong to root >&2 && exit 1)")

      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml --yes dry-update --hosts kld-00 >&2")

      # requires proper setup of certificates...
      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml --yes update --hosts kld-00 >&2")
      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml --yes update --hosts kld-00 >&2")

      hostname = installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml ssh --hosts kld-00 hostname").strip()
      assert "kld-00" == hostname, f"'kld-00' != '{hostname}'"

      installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml reboot --hosts kld-00 >&2")
      new_machine.connected = False

      # XXX find out how we can make persist more than one profile in our test
      #installer.succeed("${lib.getExe kld-mgr} --config /root/test-config.toml --yes rollback --hosts kld-00 >&2")

    '';
})
