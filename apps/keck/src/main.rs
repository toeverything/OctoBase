mod server;

use jwst_logger::init_logger;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    const PKG_NAME: &str = env!("CARGO_PKG_NAME");
    init_logger(PKG_NAME);
    jwst::print_versions(PKG_NAME, env!("CARGO_PKG_VERSION"));
    server::start_server().await;
}
