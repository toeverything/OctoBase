use std::collections::HashMap;
use std::path::PathBuf;

use swift_bridge_build::ApplePlatform as Platform;
use swift_bridge_build::{create_package, CreatePackageConfig};

fn main() {
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
