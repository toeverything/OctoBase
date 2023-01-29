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
                    .col(ColumnDef::new(Users::Name).string().not_null())
                    .col(
                        ColumnDef::new(Users::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::AvatarUrl).string())
                    .col(ColumnDef::new(Users::TokenNonce).small_integer().default(0))
                    .col(ColumnDef::new(Users::Password).string())
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp()
                            .default("CURRENT_TIMESTAMP"),
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
    Name,       // TEXT NOT NULL,
    Email,      // TEXT NOT NULL Unique,
    AvatarUrl,  // TEXT,
    TokenNonce, // SMALLINT DEFAULT 0,
    Password,   // TEXT,
    CreatedAt,  // TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
}
