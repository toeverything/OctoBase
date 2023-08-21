use crate::m20220101_000001_create_user_table::Users;

use super::m20230101_000003_create_workspaces_table::Workspaces;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Permissions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Permissions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Permissions::WorkspaceId).string().not_null())
                    .col(ColumnDef::new(Permissions::UserId).string())
                    .col(ColumnDef::new(Permissions::UserEmail).text())
                    .col(ColumnDef::new(Permissions::Type).small_integer().not_null())
                    .col(
                        ColumnDef::new(Permissions::Accepted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Permissions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("permissions_workspace_id_fkey")
                            .from(Permissions::Table, Permissions::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("permissions_user_id_fkey")
                            .from(Permissions::Table, Permissions::UserId)
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
            .drop_foreign_key(ForeignKey::drop().name("permissions_workspace_id_fkey").to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("permissions_user_id_fkey").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("permissions_workspace_id_user_id_unique").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("permissions_workspace_id_user_email_unique")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Permissions::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Permissions {
    Table,
    Id,          // STRING PRIMARY KEY,
    WorkspaceId, // CHAR(36),
    UserId,      // INTEGER,
    UserEmail,   // TEXT,
    Type,        // SMALLINT NOT NULL,
    Accepted,    // BOOL DEFAULT False,
    CreatedAt,   // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                 // FOREIGN KEY(workspace_id) REFERENCES workspaces(id),
                 // FOREIGN KEY(user_id) REFERENCES users(id),
                 // UNIQUE (workspace_id, user_id),
                 // UNIQUE (workspace_id, user_email)
}
