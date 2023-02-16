pub use sea_orm_migration::prelude::*;

mod m20220101_000001_initial_blob_table;
mod m20220101_000002_initial_doc_table;
mod schema;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial_blob_table::Migration),
            Box::new(m20220101_000002_initial_doc_table::Migration),
        ]
    }
}
