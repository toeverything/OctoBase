mod server;

use jwst_logger::init_logger;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    init_logger();
    server::start_server().await;
}
