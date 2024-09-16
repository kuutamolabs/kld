{ lib, ... }:
{
  options.kuutamo.electrs = {
    address = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Address to listen for RPC connections.";
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 50001;
      description = "Port to listen for RPC connections.";
    };
    network = lib.mkOption {
      type = lib.types.enum [ "bitcoin" "testnet" "signet" "regtest" ];
      default = "bitcoin";
      description = lib.mdDoc "Bitcoin network to use.";
    };
  };
}
