use std::{fs::create_dir_all, process::Command};

fn main() {
    if !Command::new("pnpm")
        .args(["i"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        || !Command::new("pnpm")
            .args(["run", "build"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    {
        create_dir_all("../homepage/out").unwrap();
    }
}
