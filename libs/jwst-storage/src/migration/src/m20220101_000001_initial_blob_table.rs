use super::schema::Blobs;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220101_000001_initial_blob_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Bakery table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Blobs::Table)
                    .col(ColumnDef::new(Blobs::Workspace).string().not_null())
                    .col(ColumnDef::new(Blobs::Hash).string().not_null())
                    .col(ColumnDef::new(Blobs::Blob).binary().not_null())
                    .col(ColumnDef::new(Blobs::Length).big_integer().not_null())
                    .col(
                        ColumnDef::new(Blobs::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(Index::create().col(Blobs::Workspace).col(Blobs::Hash))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("blobs_list")
                    .table(Blobs::Table)
                    .col(Blobs::Workspace)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("blobs_list").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Blobs::Table).to_owned())
            .await?;
        Ok(())
    }
}
