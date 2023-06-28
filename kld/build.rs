use paperclip::v2::{
    self,
    codegen::{DefaultEmitter, Emitter, EmitterState},
    models::{DefaultSchema, ResolvableApi},
};

use std::env;
use std::fs::File;

fn main() {
    let spec_path = "src/api/spec.yaml";
    println!("cargo:rerun-if-changed={spec_path}");

    let fd = File::open(spec_path).expect("spec not found");
    let raw: ResolvableApi<DefaultSchema> =
        v2::from_reader(fd).expect("failed to deserialise spec");
    let schema = raw.resolve().expect("resolution");

    let out_dir = env::var("OUT_DIR").unwrap();
    let mut state = EmitterState::default();
    state.mod_prefix = "crate::api::codegen::";
    state.working_dir = out_dir.into();

    let emitter = DefaultEmitter::from(state);
    emitter.generate(&schema).expect("codegen");
}
