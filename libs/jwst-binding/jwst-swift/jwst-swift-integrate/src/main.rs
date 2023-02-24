use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use swift_bridge_build::ApplePlatform as Platform;
use swift_bridge_build::{create_package, CreatePackageConfig};

fn main() {
    let common_commands = [
        "-p",
        "jwst-swift",
        "--target",
        "aarch64-apple-ios",
        "--target",
        "aarch64-apple-ios-sim",
        "--target",
        "aarch64-apple-darwin",
    ];
    Command::new("rustup")
        .args(["target", "add", "aarch64-apple-ios"])
        .status()
        .expect("Failed to add target aarch64-apple-ios");
    Command::new("rustup")
        .args(["target", "add", "aarch64-apple-ios-sim"])
        .status()
        .expect("Failed to add target aarch64-apple-ios-sim");
    Command::new("rustup")
        .args(["target", "add", "aarch64-apple-darwin"])
        .status()
        .expect("Failed to add target aarch64-apple-darwin");
    Command::new("cargo")
        .args(if cfg!(debug_assertions) {
            ["build"].iter().chain(common_commands.iter())
        } else {
            ["build", "--release"].iter().chain(common_commands.iter())
        })
        .status()
        .expect("Failed to build jwst-swift");
    create_package(CreatePackageConfig {
        bridge_dir: PathBuf::from("libs/jwst-binding/jwst-swift/generated"),
        paths: HashMap::from([
            (
                Platform::IOS,
                PathBuf::from("target/aarch64-apple-ios/debug/liboctobase.a") as _,
            ),
            (
                Platform::Simulator,
                PathBuf::from("target/aarch64-apple-ios-sim/debug/liboctobase.a") as _,
            ),
            (
                Platform::MacOS,
                PathBuf::from("target/aarch64-apple-darwin/debug/liboctobase.a") as _,
            ),
        ]),
        out_dir: PathBuf::from("apps/swift/OctoBaseSwift"),
        package_name: "OctoBase".to_string(),
    });
}
