use serde_derive::Deserialize;
use std::error::Error;
use vergen::EmitBuilder;

#[allow(dead_code)]
#[derive(Deserialize)]
struct SystemInfo {
    git_sha: String,
    git_commit_date: String,
    project_hash: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    if let Ok(content) = std::fs::read_to_string("../system-info.toml") {
        if let Ok(system_info) = toml::from_str::<SystemInfo>(&content) {
            println!("cargo:rustc-env=VERGEN_GIT_SHA={}", system_info.git_sha);
            println!(
                "cargo:rustc-env=VERGEN_GIT_COMMIT_DATE={}",
                system_info.git_commit_date
            );
            Ok(())
        } else {
            eprintln!("system-info format incorrect");
            Err("system-info format incorrect".into())
        }
    } else {
        EmitBuilder::builder()
            .git_commit_date()
            .git_sha(false)
            .emit()?;
        Ok(())
    }
}
