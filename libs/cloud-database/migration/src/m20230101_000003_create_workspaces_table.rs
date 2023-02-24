use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Workspaces::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Workspaces::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Workspaces::Public).boolean().not_null())
                    .col(ColumnDef::new(Workspaces::Type).small_integer().not_null())
                    .col(
                        ColumnDef::new(Workspaces::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Workspaces::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Workspaces {
    Table,
    Id,        // STRING PRIMARY KEY,
    Public,    // BOOL NOT NULL,
    Type,      // SMALLINT NOT NULL,
    CreatedAt, // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
}
