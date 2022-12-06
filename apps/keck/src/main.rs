mod server;

use jwst_logger::{init_logger, Level};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_logger(Level::Debug)?;
    server::start_server().await;

    Ok(())
}
