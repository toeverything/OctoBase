use mdbook::MDBook;
use std::path::PathBuf;

fn main() {
    let root_dir = "../handbook";

    let md = MDBook::load(root_dir).expect("Unable to load the book");
    md.build().expect("Building failed");
    println!(
        "cargo:rerun-if-changed={}",
        PathBuf::from(root_dir).join("src").display()
    );
}
