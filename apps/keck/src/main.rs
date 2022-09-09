#![feature(async_closure)]

mod server;
mod sync;
mod utils;

use utils::*;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_logger(Level::Debug)?;
    server::start_server().await;

    Ok(())
}
