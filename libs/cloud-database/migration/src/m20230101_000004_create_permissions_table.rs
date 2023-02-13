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
                    .col(
                        ColumnDef::new(Permissions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Permissions::WorkspaceId).char_len(36))
                    .col(ColumnDef::new(Permissions::UserId).char_len(36))
                    .col(ColumnDef::new(Permissions::UserEmail).text())
                    .col(ColumnDef::new(Permissions::Type).small_integer().not_null())
                    .col(
                        ColumnDef::new(Permissions::Accepted)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Permissions::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("permissions_workspace_id_fkey")
                            .from(Permissions::Table, Permissions::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Uuid)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("permissions_user_id_fkey")
                            .from(Permissions::Table, Permissions::UserId)
                            .to(Users::Table, Users::Uuid)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("permissions_workspace_id_fkey")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("permissions_user_id_fkey")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("permissions_workspace_id_user_id_unique")
                    .to_owned(),
            )
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
enum Permissions {
    Table,
    Id,          // BIGSERIAL PRIMARY KEY,
    WorkspaceId, // CHAR(36),
    UserId,      // INTEGER,
    UserEmail,   // TEXT,
    Type,        // SMALLINT NOT NULL,
    Accepted,    // BOOL DEFAULT False,
    CreatedAt,   // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                 // FOREIGN KEY(workspace_id) REFERENCES workspaces(uuid),
                 // FOREIGN KEY(user_id) REFERENCES users(uuid),
                 // UNIQUE (workspace_id, user_id),
                 // UNIQUE (workspace_id, user_email)
}
