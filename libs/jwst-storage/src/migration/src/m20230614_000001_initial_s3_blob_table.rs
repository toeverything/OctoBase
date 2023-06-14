use super::schema::S3Blobs;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230614_000001_initial_s3_blob_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Bakery table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(S3Blobs::Table)
                    .col(
                        ColumnDef::new(S3Blobs::Workspace)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(S3Blobs::Hash).string().not_null())
                    .col(
                        ColumnDef::new(S3Blobs::Length)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(S3Blobs::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(S3Blobs::Params).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(S3Blobs::Workspace)
                            .col(S3Blobs::Hash)
                            .col(S3Blobs::Params),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("s3_blobs_list")
                    .table(S3Blobs::Table)
                    .col(S3Blobs::Workspace)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("s3_blobs_list").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(S3Blobs::Table).to_owned())
            .await?;
        Ok(())
    }
}
