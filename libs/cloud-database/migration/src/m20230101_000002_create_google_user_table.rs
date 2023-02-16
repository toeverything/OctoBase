use super::m20220101_000001_create_user_table::Users;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GoogleUsers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GoogleUsers::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GoogleUsers::UserId).string().not_null())
                    .col(
                        ColumnDef::new(GoogleUsers::GoogleId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("google_users_user_id_fkey")
                            .from(GoogleUsers::Table, GoogleUsers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("google_users_user_id_fkey")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(GoogleUsers::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum GoogleUsers {
    Table,
    Id,       // STRING PRIMARY KEY,
    UserId,   // INTEGER REFERENCES users(id),
    GoogleId, // TEXT NOT NULL UNIQUE (google_id)
}
