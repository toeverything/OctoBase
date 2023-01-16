use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(jwst_blob_migration::Migrator).await;
}
