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
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GoogleUsers::UserId)
                            .integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(GoogleUsers::GoogleId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("google_users_user_id_fkey")
                            .from(Users::Table, Users::Id)
                            .to(GoogleUsers::Table, GoogleUsers::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GoogleUsers::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum GoogleUsers {
    Table,
    Id,       // SERIAL PRIMARY KEY,
    UserId,   // INTEGER REFERENCES users(id),
    GoogleId, // TEXT NOT NULL UNIQUE (google_id)
}
