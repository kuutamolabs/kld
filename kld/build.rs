fn main() {
    if let Some(commit_sha) = option_env!("COMMIT_SHA") {
        println!("cargo:rustc-env=COMMIT_SHA={commit_sha}");
    }
}
