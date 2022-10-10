use std::path::PathBuf;

fn main() {
    #[cfg(feature = "docs")]
    {
        let root_dir = "../handbook";

        let md = mdbook::MDBook::load(root_dir).expect("Unable to load the book");
        md.build().expect("Building failed");
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(root_dir).join("src").display()
        );
    }
}
