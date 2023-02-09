use std::{collections::HashMap, env, path::PathBuf};

const XCODE_CONFIGURATION_ENV: &'static str = "CONFIGURATION";

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let output = PathBuf::from(
        env::var("CARGO_TARGET_DIR").unwrap_or(env::var("CARGO_MANIFEST_DIR").unwrap()),
    )
    .join("binding.h");

    let rename = {
        let mut map = HashMap::new();

        map.insert("Workspace".to_string(), "JWSTWorkspace".to_string());
        map.insert("Block".to_string(), "JWSTBlock".to_string());
        map.insert("Transaction".to_string(), "YTransaction".to_string());

        map
    };

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(cbindgen::Config {
            language: cbindgen::Language::C,
            header: Some(String::from(
                r#"
#ifndef JWST_FFI_H
#define JWST_FFI_H
typedef struct JWSTWorkspace {} JWSTWorkspace;
typedef struct JWSTBlock {} JWSTBlock;
typedef struct YTransaction {} YTransaction;
"#,
            )),
            trailer: Some(String::from(
                r#"
#endif            
"#,
            )),
            export: cbindgen::ExportConfig {
                rename,
                ..Default::default()
            },
            ..Default::default()
        })
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(output);

    let out_dir = "../../../apps/swift/octobase/Generated";

    let bridges = vec!["src/lib.rs"];
    for path in &bridges {
        println!("cargo:rerun-if-changed={}", path);
    }
    println!("cargo:rerun-if-env-changed={}", XCODE_CONFIGURATION_ENV);

    swift_bridge_build::parse_bridges(bridges)
        .write_all_concatenated(out_dir, env!("CARGO_PKG_NAME"));
}
