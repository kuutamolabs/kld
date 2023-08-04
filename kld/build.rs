use clap::CommandFactory;
use paperclip::v2::{
    self,
    codegen::{DefaultEmitter, Emitter, EmitterState},
    models::{DefaultSchema, ResolvableApi},
};

use std::env;
use std::fs::File;

use clap_complete::{generate_to, shells::Bash};

include!("src/cli/commands.rs");

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    generate_api(out_dir.clone());
    generate_cli_completion(out_dir);
}

fn generate_cli_completion(out_dir: String) {
    let mut cmd = KldCliCommand::command();
    let _path = generate_to(Bash, &mut cmd, "kld-cli", out_dir).unwrap();

    println!("cargo:rerun-if-changed=src/cli/commands.rs");
}

fn generate_api(out_dir: String) {
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
