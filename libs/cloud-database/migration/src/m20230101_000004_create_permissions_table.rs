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
                    .table(Permission::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Permission::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Permission::WorkspaceId)
                            .char_len(36)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Permission::UserId).integer().not_null())
                    .col(ColumnDef::new(Permission::UserEmail).text().not_null())
                    .col(ColumnDef::new(Permission::Type).small_integer().not_null())
                    .col(
                        ColumnDef::new(Permission::Accepted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Permission::CreatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("permission_workspace_id_fkey")
                            .from(Workspaces::Table, Workspaces::Uuid)
                            .to(Permission::Table, Permission::WorkspaceId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("permission_user_id_fkey")
                            .from(Users::Table, Users::Id)
                            .to(Permission::Table, Permission::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("permission_workspace_id_user_id_unique")
                            .col(Permission::WorkspaceId)
                            .col(Permission::UserId),
                    )
                    .index(
                        Index::create()
                            .name("permission_workspace_id_user_email_unique")
                            .col(Permission::WorkspaceId)
                            .col(Permission::UserEmail),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("permission_workspace_id_fkey")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("permission_user_id_fkey")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("permission_workspace_id_user_id_unique")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("permission_workspace_id_user_email_unique")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Permission::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Permission {
    Table,
    Id,          // BIGSERIAL PRIMARY KEY,
    WorkspaceId, // CHAR(36),
    UserId,      // INTEGER,
    UserEmail,   // TEXT,
    Type,        // SMALLINT NOT NULL,
    Accepted,    // BOOL DEFAULT False,
    CreatedAt,   // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                 // FOREIGN KEY(workspace_id) REFERENCES workspaces(uuid),
                 // FOREIGN KEY(user_id) REFERENCES users(id),
                 // UNIQUE (workspace_id, user_id),
                 // UNIQUE (workspace_id, user_email)
}
