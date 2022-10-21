use std::{env, path::PathBuf};

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let output = PathBuf::from(
        env::var("CARGO_TARGET_DIR").unwrap_or(env::var("CARGO_MANIFEST_DIR").unwrap()),
    )
    .join("binding.h");

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(output);
}
