{ lib, ... }:
{
  options = {
    kuutamo.telegraf.url = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "url to remote monitor";
    };

    kuutamo.telegraf.username = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "username to remote monitor";
    };

    kuutamo.telegraf.password = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "password to remote monitor";
    };

    kuutamo.telegraf.kmonitoring_url = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "url to kuutamo monitor";
    };

    kuutamo.telegraf.kmonitoring_protocol = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "protocol to kuutamo monitor";
    };

    kuutamo.telegraf.kmonitoring_user_id = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "user_id for kuutamo monitor";
    };

    kuutamo.telegraf.kmonitoring_password = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "password for kuutamo monitor";
    };
  };
}
