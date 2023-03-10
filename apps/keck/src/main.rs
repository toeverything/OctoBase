mod server;

use jwst_logger::{init_logger, print_versions};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    init_logger();
    print_versions(env!("CARGO_PKG_VERSION"));
    server::start_server().await;
}
