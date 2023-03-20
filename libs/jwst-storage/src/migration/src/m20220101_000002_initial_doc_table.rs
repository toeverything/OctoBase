use super::schema::Docs;
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
        println!("Migration: create docs table");
        manager
            .create_table(
                Table::create()
                    .table(Docs::Table)
                    .col(
                        ColumnDef::new(Docs::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Docs::Workspace).string().not_null())
                    .col(
                        ColumnDef::new(Docs::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Docs::Blob).binary().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("workspaces_update")
                    .table(Docs::Table)
                    .col(Docs::Workspace)
                    .to_owned(),
            )
            .await?;

        println!("Migration: create docs table finished");
        Ok(())
    }

    // Define how to rollback this migration: Drop the Bakery table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("workspaces_update").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Docs::Table).to_owned())
            .await?;
        Ok(())
    }
}
