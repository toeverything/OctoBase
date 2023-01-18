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
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Workspaces::Uuid)
                            .string_len(36)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Workspaces::Public).boolean().not_null())
                    .col(ColumnDef::new(Workspaces::Type).small_integer().not_null())
                    .col(ColumnDef::new(Workspaces::CreatedAt).timestamp().not_null())
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
enum Workspaces {
    Table,
    Id,        // BIGSERIAL PRIMARY KEY,
    Uuid,      // CHAR(36) UNIQUE(uuid),
    Public,    // BOOL NOT NULL,
    Type,      // SMALLINT NOT NULL,
    CreatedAt, // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
}
