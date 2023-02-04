use super::schema::UpdateBinary;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220101_000001_initial_doc_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Bakery table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UpdateBinary::Table)
                    .col(
                        ColumnDef::new(UpdateBinary::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UpdateBinary::Workspace).string().not_null())
                    .col(
                        ColumnDef::new(UpdateBinary::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UpdateBinary::Blob).binary().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("workspaces_update")
                    .table(UpdateBinary::Table)
                    .col(UpdateBinary::Workspace)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("workspaces_update").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UpdateBinary::Table).to_owned())
            .await?;
        Ok(())
    }
}
