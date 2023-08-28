use std::{env, fs, path::Path};

use flapigen::{JavaConfig, LanguageConfig};
use rifgen::{Generator, Language, TypeCases};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let in_src = "src/java_glue.rs.in";

    Generator::new(TypeCases::CamelCase, Language::Java, "src").generate_interface(in_src);

    let template = fs::read_to_string(in_src).unwrap();
    let template = template.split("use jni_sys::*;").collect::<Vec<_>>();
    let template = template
        .first()
        .into_iter()
        .chain(["use jni_sys::*;"].iter())
        .chain(
            [
                r#"foreign_class!(
    class JwstStorage {
        self_type JwstStorage;
        constructor JwstStorage::new(path: String) -> JwstStorage;
        constructor JwstStorage::new_with_logger_level(path: String, level: String) -> JwstStorage;
        fn JwstStorage::error(&self) -> Option<String>; alias error;
        fn JwstStorage::is_offline(&self) -> bool;
        fn JwstStorage::is_connected(&self) -> bool;
        fn JwstStorage::is_finished(&self) -> bool;
        fn JwstStorage::is_error(&self) -> bool;
        fn JwstStorage::get_sync_state(&self) -> String;
        fn JwstStorage::connect(&mut self, workspace_id: String, remote: String) -> Option<Workspace>; alias connect;
        fn JwstStorage::get_last_synced(&self) ->Vec<i64>;
    }
);"#,
                r#"
pub type VecOfStrings = Vec<String>;
foreign_class!(
    class VecOfStrings {
        self_type VecOfStrings;

        constructor default() -> VecOfStrings {
            Vec::<String>::default()
        }

        fn at(&self, i: usize) -> &str {
            this[i].as_str()
        }

        fn len(&self) -> usize {
            this.len()
        }

        fn push(&mut self, s: String) {
            this.push(s);
        }

        fn insert(&mut self, i: usize, s: String) {
            this.insert(i, s);
        }

        fn clear(&mut self) {
            this.clear();
        }

        fn remove(&mut self, i: usize) {
            this.remove(i);
        }

        fn remove_item(&mut self, s: String) {
            this.retain(|x| x != &s);
        }
    }
);"#,
            ]
            .iter(),
        )
        .chain(template.iter().skip(1))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(in_src, template).unwrap();

    //delete the lib folder then create it again to prevent obsolete files from
    // staying
    let java_folder = Path::new("android").join("src/main/java/com/toeverything/jwst/lib");
    if java_folder.exists() {
        fs::remove_dir_all(&java_folder).unwrap();
    }
    fs::create_dir(&java_folder).unwrap();
    let swig_gen = flapigen::Generator::new(LanguageConfig::JavaConfig(
        JavaConfig::new(java_folder, "com.toeverything.jwst.lib".into())
            .use_null_annotation_from_package("androidx.annotation".into()),
    ))
    .rustfmt_bindings(true);

    swig_gen.expand("android bindings", in_src, Path::new(&out_dir).join("java_glue.rs"));

    println!("cargo:rerun-if-changed=src");
}
