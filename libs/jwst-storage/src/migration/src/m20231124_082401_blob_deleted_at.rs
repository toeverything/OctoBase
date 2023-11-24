use sea_orm_migration::prelude::*;

use crate::schemas::{Blobs, OptimizedBlobs};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Blobs::Table)
                    .add_column(ColumnDef::new(Blobs::DeletedAt).timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(OptimizedBlobs::Table)
                    .add_column(
                        ColumnDef::new(OptimizedBlobs::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Blobs::Table)
                    .drop_column(Blobs::DeletedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(OptimizedBlobs::Table)
                    .drop_column(OptimizedBlobs::DeletedAt)
                    .to_owned(),
            )
            .await
    }
}
