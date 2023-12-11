use clap::CommandFactory;
use paperclip::v2::{
    self,
    codegen::{DefaultEmitter, Emitter, EmitterState},
    models::{DefaultSchema, ResolvableApi},
};

use std::env;
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string, File, OpenOptions};
use std::io::Write;

use clap_complete::{generate_to, shells::Bash};

include!("src/cli/commands.rs");

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    generate_api(&out_dir);
    patch_api(&out_dir);
    generate_cli_completion(&out_dir);
}

fn generate_cli_completion(out_dir: &String) {
    let mut cmd = KldCliCommand::command();
    let _path = generate_to(Bash, &mut cmd, "kld-cli", out_dir).unwrap();

    println!("cargo:rerun-if-changed=src/cli/commands.rs");
}

fn generate_api(out_dir: &String) {
    let spec_path = "src/api/spec.yaml";

    let fd = File::open(spec_path).expect("spec not found");
    let raw: ResolvableApi<DefaultSchema> =
        v2::from_reader(fd).expect("failed to deserialise spec");
    let schema = raw.resolve().expect("resolution");

    let mut state = EmitterState::default();
    state.mod_prefix = "crate::api::codegen::";
    state.working_dir = out_dir.into();

    let emitter = DefaultEmitter::from(state);
    emitter.generate(&schema).expect("codegen");

    println!("cargo:rerun-if-changed={spec_path}");
}

/// Patch with unsigned integer based types, which are blockchain native
/// Also handle timestamp and user_channel_id types
fn patch_api(out_dir: &String) {
    for entry in read_dir(out_dir)
        .expect("OUT_DIR is expected with cargo build")
        .flatten()
    {
        let path = entry.path();
        if path.extension() == Some(OsStr::new("rs")) {
            let contents = read_to_string(&path).expect("Can not read rust file under OUT_DIR");
            let new = contents
                .replace("user_channel_id: i64", "user_channel_id: u128")
                .replace(
                    "user_channel_id(mut self, value: impl Into<i64>)",
                    "user_channel_id(mut self, value: impl Into<u128>)",
                )
                .replace(
                    "force_close_spend_delay: Option<i64>",
                    "force_close_spend_delay: Option<u16>",
                )
                .replace(
                    "force_close_spend_delay(mut self, value: impl Into<i64>)",
                    "force_close_spend_delay(mut self, value: impl Into<u16>)",
                )
                .replace(
                    "config_cltv_expiry_delta: i64",
                    "config_cltv_expiry_delta: u16",
                )
                .replace(
                    "config_cltv_expiry_delta(mut self, value: impl Into<i64>)",
                    "config_cltv_expiry_delta(mut self, value: impl Into<u16>)",
                )
                .replace("i64", "u64")
                .replace("i32", "u32")
                .replace("_timestamp: u64", "_timestamp: i64")
                .replace("_timestamp: Option<u64>", "_timestamp: Option<i64>")
                .replace(
                    "_timestamp(mut self, value: impl Into<u64>)",
                    "_timestamp(mut self, value: impl Into<i64>)",
                );
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .expect("Can not reopen rust file under OUT_DIR");
            let _ = file
                .write(new.as_bytes())
                .expect("Can not rewrite rust file under OUT_DIR");
        }
    }
}
