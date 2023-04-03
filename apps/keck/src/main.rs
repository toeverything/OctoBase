mod server;

use jwst_logger::init_logger;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    init_logger();
    jwst::print_versions(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    server::start_server().await;
}
