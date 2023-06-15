use super::schema::BucketBlobs;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230614_000001_initial_bucket_blob_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Bakery table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BucketBlobs::Table)
                    .col(ColumnDef::new(BucketBlobs::Workspace).string().not_null())
                    .col(ColumnDef::new(BucketBlobs::Hash).string().not_null())
                    .col(ColumnDef::new(BucketBlobs::Length).big_integer().not_null())
                    .col(
                        ColumnDef::new(BucketBlobs::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(BucketBlobs::Workspace)
                            .col(BucketBlobs::Hash),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("bucket_blobs_list")
                    .table(BucketBlobs::Table)
                    .col(BucketBlobs::Workspace)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("bucket_blobs_list").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BucketBlobs::Table).to_owned())
            .await?;
        Ok(())
    }
}
