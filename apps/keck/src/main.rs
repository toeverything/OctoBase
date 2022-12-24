mod server;

use jwst_logger::{init_logger, Level};

#[tokio::main]
async fn main() {
    init_logger();
    server::start_server().await;
}
