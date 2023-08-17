use sea_orm_migration::prelude::*;

use crate::schema::Docs;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Docs::Table)
                    .add_column(ColumnDef::new(Docs::Guid).string().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Docs::Table)
                    .add_column(
                        ColumnDef::new(Docs::IsWorkspace)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Docs::Table)
                    .name("docs_guid")
                    .col(Docs::Guid)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("docs_guid").to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Docs::Table)
                    .drop_column(Docs::Guid)
                    .drop_column(Docs::IsWorkspace)
                    .to_owned(),
            )
            .await
    }
}
