use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(jwst_storage_migration::Migrator).await;
}
