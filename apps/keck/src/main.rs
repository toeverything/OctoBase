mod server;

use jwst_logger::init_logger;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() {
    // ignore load error when missing env file
    let _ = dotenvy::dotenv();
    init_logger(PKG_NAME);
    jwst::print_versions(PKG_NAME, env!("CARGO_PKG_VERSION"));
    server::start_server().await;
}
