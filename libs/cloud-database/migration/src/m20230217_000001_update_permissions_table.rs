use crate::m20220101_000001_create_user_table::Users;
use crate::m20230101_000004_create_permissions_table::Permissions;

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
            .column(Users::Email)
            .from(Users::Table)
            .to_owned();
        let rows = trx.query_all(builder.build(&stmt)).await?;

        for row in rows.into_iter() {
            let user_id = row.try_get::<String>("", "id").unwrap();
            let user_email = row.try_get::<String>("", "email").unwrap();

            let stmt = Query::update()
                .table(Permissions::Table)
                .values(vec![(Permissions::UserId, user_id.into())])
                .cond_where(Cond::all().add(Expr::col(Permissions::UserEmail).eq(user_email)))
                .to_owned();

            trx.execute(builder.build(&stmt)).await?;
        }

        trx.commit().await?;

        Ok(())
    }
}
