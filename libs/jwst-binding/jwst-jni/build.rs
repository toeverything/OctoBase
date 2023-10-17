use std::{env, fs, path::Path};

use flapigen::{JavaConfig, LanguageConfig};
use rifgen::{Generator, Language, TypeCases};

fn main() {
    let jni_dir = Path::new("android").join("src/main/java/com/toeverything/jwst/lib");
    let out_dir = env::var("OUT_DIR").unwrap();
    let in_temp = Path::new(&out_dir).join("java_glue.rs.in");
    let in_src = Path::new("src").join("java_glue.rs.in");

    Generator::new(TypeCases::CamelCase, Language::Java, "src").generate_interface(&in_temp);

    let template = fs::read_to_string(&in_temp).unwrap();
    let template = template.split("use jni_sys::*;").collect::<Vec<_>>();
    let template = template
        .first()
        .into_iter()
        .chain(["use jni_sys::*;"].iter())
        .chain(
            [
                r#"foreign_typemap!(
    ($p:r_type) Vec<u8> => jbyteArray {
        let slice = &($p)[..];
        let slice = unsafe { std::mem::transmute::<&[u8], &[i8]>(slice) };
        let raw = JavaByteArray::from_slice_to_raw(slice, env);
        $out = raw;
    };
    ($p:f_type) => "jbyteArray";
);

foreign_typemap!(
    ($p:r_type) Vec<u8> => jbyteArray {
        let slice = &($p)[..];
        let slice = unsafe { std::mem::transmute::<&[u8], &[i8]>(slice) };
        let raw = JavaByteArray::from_slice_to_raw(slice, env);
        $out = raw;
    };
    ($p:f_type) => "jbyteArray";
    ($p:r_type) &'a [u8] <= jbyteArray {
        let arr = JavaByteArray::new(env, $p);
        let slice = arr.to_slice();
        let slice = unsafe { std::mem::transmute::<&[i8], &[u8]>(slice) };
        $out = slice;
    };
    ($p:f_type) <= "jbyteArray";
);"#,
                r#"foreign_class!(
    class JwstStorage {
        self_type JwstStorage;
        constructor JwstStorage::new(path: String) -> JwstStorage;
        constructor JwstStorage::new_with_log_level(path: String, level: String) -> JwstStorage;
        fn JwstStorage::error(&self) -> Option<String>; alias error;
        fn JwstStorage::is_offline(&self) -> bool;
        fn JwstStorage::is_connected(&self) -> bool;
        fn JwstStorage::is_finished(&self) -> bool;
        fn JwstStorage::is_error(&self) -> bool;
        fn JwstStorage::get_sync_state(&self) -> String;
        fn JwstStorage::init(&mut self, workspace_id: String, data: &[u8]) -> bool; alias init;
        fn JwstStorage::export(&mut self, workspace_id: String) -> Vec<u8>; alias export;
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
    fs::write(&in_temp, &template).unwrap();

    let template_changed = fs::read_to_string(&in_src).unwrap() != template;

    if template_changed || !in_temp.with_extension("").exists() || !jni_dir.exists() {
        // delete the lib folder then create it again to prevent obsolete files
        // from staying
        if jni_dir.exists() {
            fs::remove_dir_all(&jni_dir).unwrap();
        }
        fs::create_dir(&jni_dir).unwrap();
        let swig_gen = flapigen::Generator::new(LanguageConfig::JavaConfig(
            JavaConfig::new(jni_dir, "com.toeverything.jwst.lib".into())
                .use_null_annotation_from_package("androidx.annotation".into()),
        ))
        .rustfmt_bindings(true);

        swig_gen.expand("android bindings", &in_temp, in_temp.with_extension("out"));

        if !in_temp.with_extension("").exists()
            || fs::read_to_string(in_temp.with_extension("out")).unwrap()
                != fs::read_to_string(in_temp.with_extension("")).unwrap()
        {
            fs::copy(in_temp.with_extension("out"), in_temp.with_extension("")).unwrap();
            // fs::copy(in_temp.with_extension("out"), in_src).unwrap();
        }
    }
}
