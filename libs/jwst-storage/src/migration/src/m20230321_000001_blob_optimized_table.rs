use super::schema::OptimizedBlobs;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230321_000001_blob_optimized_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Bakery table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OptimizedBlobs::Table)
                    .col(
                        ColumnDef::new(OptimizedBlobs::Workspace)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OptimizedBlobs::Hash).string().not_null())
                    .col(ColumnDef::new(OptimizedBlobs::Blob).binary().not_null())
                    .col(
                        ColumnDef::new(OptimizedBlobs::Length)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OptimizedBlobs::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OptimizedBlobs::Params).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(OptimizedBlobs::Workspace)
                            .col(OptimizedBlobs::Hash)
                            .col(OptimizedBlobs::Params),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("blobs_optimized_list")
                    .table(OptimizedBlobs::Table)
                    .col(OptimizedBlobs::Workspace)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("blobs_optimized_list").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OptimizedBlobs::Table).to_owned())
            .await?;
        Ok(())
    }
}
