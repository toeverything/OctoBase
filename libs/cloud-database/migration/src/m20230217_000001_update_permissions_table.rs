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

        let mut stmt = Query::select();
        stmt.column(Users::Id);
        stmt.column(Users::Email);
        stmt.from(Users::Table);
        let rows = db.query_all(builder.build(&stmt)).await?;

        for row in rows.into_iter() {
            let user_id = row.try_get::<String>("", "id").unwrap();
            let user_email = row.try_get::<String>("", "email").unwrap();
            let mut set_values = vec![];
            set_values.push((Permissions::UserId, user_id.into()));
            let mut condition = Cond::all();
            condition = condition.add(Expr::col(Permissions::UserEmail).eq(user_email));
            let stmt = Query::update()
                .table(Permissions::Table)
                .values(set_values)
                .cond_where(condition)
                .to_owned();
            db.execute(builder.build(&stmt)).await?;
        }
        Ok(())
    }
}
