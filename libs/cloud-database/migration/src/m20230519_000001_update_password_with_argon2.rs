use crate::m20220101_000001_create_user_table::Users;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::query::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let builder = db.get_database_backend();

        let trx = db.begin().await?;

        let stmt = Query::select()
            .column(Users::Id)
            .column(Users::Password)
            .from(Users::Table)
            .to_owned();
        let rows = trx.query_all(builder.build(&stmt)).await?;

        for row in rows.into_iter() {
            let user_id = row.try_get::<String>("", "id").unwrap();
            let user_password = row.try_get::<String>("", "password").unwrap();

            let salt = SaltString::generate(&mut OsRng);

            // Argon2 with default params (Argon2id v19)
            let hashed_password = Argon2::default()
                .hash_password(user_password.as_bytes(), &salt)
                .or(Err(DbErr::RecordNotUpdated))?
                .to_string();

            let stmt = Query::update()
                .table(Users::Table)
                .values(vec![(Users::Password, hashed_password.into())])
                .cond_where(Cond::all().add(Expr::col(Users::Id).eq(user_id)))
                .to_owned();

            trx.execute(builder.build(&stmt)).await?;
        }

        trx.commit().await?;

        Ok(())
    }
}
