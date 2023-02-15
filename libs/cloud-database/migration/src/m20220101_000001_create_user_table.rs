use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Users::Uuid).char_len(36).unique_key())
                    .col(ColumnDef::new(Users::Name).text().not_null())
                    .col(ColumnDef::new(Users::Email).text().not_null().unique_key())
                    .col(ColumnDef::new(Users::AvatarUrl).text())
                    .col(ColumnDef::new(Users::TokenNonce).small_integer().default(0))
                    .col(ColumnDef::new(Users::Password).text())
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Users {
    Table,
    Id,         // SERIAL PRIMARY KEY,
    Uuid,       // CHAR(36),
    Name,       // TEXT NOT NULL,
    Email,      // TEXT NOT NULL Unique,
    AvatarUrl,  // TEXT,
    TokenNonce, // SMALLINT DEFAULT 0,
    Password,   // TEXT,
    CreatedAt,  // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
}
