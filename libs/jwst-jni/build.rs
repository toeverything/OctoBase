use flapigen::{JavaConfig, LanguageConfig};
use rifgen::{Generator, Language, TypeCases};
use std::path::Path;
use std::{env, fs};

fn main() {
    let root = env::var("CARGO_WORKSPACE_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let in_src = "src/java_glue.rs.in";

    Generator::new(TypeCases::CamelCase, Language::Java, "src").generate_interface(in_src);

    //delete the lib folder then create it again to prevent obsolete files from staying
    let java_folder = Path::new(&root)
        .join("apps")
        .join("android")
        .join("app/src/main/java/com/example/jwst_demo/lib");
    if java_folder.exists() {
        fs::remove_dir_all(&java_folder).unwrap();
    }
    fs::create_dir(&java_folder).unwrap();
    let swig_gen = flapigen::Generator::new(LanguageConfig::JavaConfig(
        JavaConfig::new(java_folder.into(), "com.example.jwst_demo.lib".into())
            .use_null_annotation_from_package("androidx.annotation".into()),
    ))
    .rustfmt_bindings(true);

    swig_gen.expand(
        "android bindings",
        in_src,
        &Path::new(&out_dir).join("java_glue.rs"),
    );

    println!("cargo:rerun-if-changed=src");
}
