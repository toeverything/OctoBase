#[forbid(unsafe_code)]
use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(affine_cloud_migration::Migrator).await;
}
