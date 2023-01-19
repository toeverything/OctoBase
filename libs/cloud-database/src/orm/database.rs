use super::{
    model::{CreateUser, RefreshToken, User, UserLogin, Workspace, WorkspaceDetail, WorkspaceType},
    *,
};
use affine_cloud_migration::{JoinType, Migrator, MigratorTrait};
use chrono::{NaiveDateTime, Utc};
use path_ext::PathExt;
use sea_orm::{
    prelude::*, ConnectionTrait, Database, DatabaseTransaction, QuerySelect, Set, TransactionTrait,
};
use std::{
    io::{self, Cursor},
    path::PathBuf,
};
use uuid::Uuid;

pub enum PermissionType {
    Read = 0,
    Write = 1,
    Admin = 10,
    Owner = 99,
}

// #[derive(FromRow)]
// struct PermissionQuery {
//     #[sqlx(rename = "type")]
//     type_: PermissionType,
// }

pub struct ORM {
    pub pool: DatabaseConnection,
}

impl ORM {
    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self { pool })
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UsersModel>, DbErr> {
        Users::find()
            .filter(UsersColumn::Email.eq(email))
            .one(&self.pool)
            .await
    }

    pub async fn get_workspace_owner(
        &self,
        workspace_id: String,
    ) -> Result<Option<UsersModel>, DbErr> {
        Permission::find()
            .column(UsersColumn::Id)
            .column(UsersColumn::Name)
            .column(UsersColumn::Email)
            .column(UsersColumn::AvatarUrl)
            .column(UsersColumn::CreatedAt)
            .join_rev(
                JoinType::InnerJoin,
                Users::belongs_to(Permission)
                    .from(UsersColumn::Id)
                    .to(PermissionColumn::UserId)
                    .into(),
            )
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .filter(PermissionColumn::Type.eq(PermissionType::Owner as i16))
            .into_model::<UsersModel>()
            .one(&self.pool)
            .await
    }

    pub async fn user_login(&self, login: UserLogin) -> Result<Option<UsersModel>, DbErr> {
        Users::find()
            .filter(UsersColumn::Email.eq(login.email))
            .filter(UsersColumn::Password.eq(login.password))
            .one(&self.pool)
            .await
    }

    pub async fn refresh_token(&self, token: RefreshToken) -> Result<Option<UsersModel>, DbErr> {
        Users::find()
            .filter(UsersColumn::Id.eq(token.user_id))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
    }

    pub async fn verify_refresh_token(&self, token: &RefreshToken) -> Result<bool, DbErr> {
        Users::find()
            .column(UsersColumn::Id)
            .filter(UsersColumn::Id.eq(token.user_id))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
            .map(|r| r.is_some())
    }

    pub async fn update_cred(
        trx: &DatabaseTransaction,
        user_id: i32,
        user_email: &str,
    ) -> Result<(), DbErr> {
        Permission::update(PermissionActiveModel {
            user_id: Set(user_id),
            user_email: Set(None),
            ..Default::default()
        })
        .filter(PermissionColumn::UserEmail.eq(user_email))
        .exec(trx)
        .await
        .map(|_| ())
    }

    pub async fn create_user(
        &self,
        user: CreateUser,
    ) -> Result<Option<(UsersModel, WorkspacesModel)>, DbErr> {
        let mut trx = self.pool.begin().await?;

        let Ok(user) = Users::insert(UsersActiveModel {
            name: Set(user.name),
            password: Set(Some(user.password)),
            email: Set(user.email),
            avatar_url: Set(user.avatar_url),
            ..Default::default()
        })
        .exec_with_returning(&trx)
        .await else {
            return Ok(None);
        };

        let new_workspace = self
            .create_workspace(&trx, user.id, WorkspaceType::Private)
            .await?;

        Self::update_cred(&mut trx, user.id, &user.email).await?;

        trx.commit().await?;

        Ok(Some((user, new_workspace)))
    }

    pub async fn get_workspace_by_id(
        &self,
        workspace_id: String,
    ) -> Result<Option<WorkspaceDetail>, DbErr> {
        let workspace = Workspaces::find()
            .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
            .one(&self.pool)
            .await?;

        let workspace = match workspace {
            Some(workspace) if workspace.r#type == WorkspaceType::Private as i32 => {
                return Ok(Some(WorkspaceDetail {
                    owner: None,
                    member_count: 0,
                    workspace: Workspace {
                        id: workspace.uuid.clone(),
                        public: workspace.public,
                        r#type: workspace.r#type.into(),
                        created_at: workspace.created_at.naive_local(),
                    },
                }))
            }
            Some(ws) => ws,
            None => return Ok(None),
        };

        let owner = self
            .get_workspace_owner(workspace_id.clone())
            .await?
            .expect("owner not found");

        let get_member_count = "SELECT COUNT(permissions.id) AS count
            FROM permissions
            WHERE workspace_id = $1 AND accepted = True";

        let member_count = Permission::find()
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .filter(PermissionColumn::Accepted.eq(true))
            .count(&self.pool)
            .await?;

        Ok(Some(WorkspaceDetail {
            owner: Some(User {
                id: owner.id,
                name: owner.name,
                email: owner.email,
                avatar_url: owner.avatar_url,
                created_at: owner.created_at.unwrap_or_default().naive_local(),
            }),
            member_count,
            workspace: Workspace {
                id: workspace.uuid.clone(),
                public: workspace.public,
                r#type: workspace.r#type.into(),
                created_at: workspace.created_at.naive_local(),
            },
        }))
    }

    pub async fn create_workspace<C: ConnectionTrait>(
        &self,
        trx: &C,
        user_id: i32,
        ws_type: WorkspaceType,
    ) -> Result<WorkspacesModel, DbErr> {
        let uuid = Uuid::new_v4();
        let workspace_id = uuid.to_string().replace("-", "_");

        let workspace = Workspaces::insert(WorkspacesActiveModel {
            public: Set(false),
            r#type: Set(ws_type as i32),
            uuid: Set(workspace_id),
            ..Default::default()
        })
        .exec_with_returning(trx)
        .await?;

        Permission::insert(PermissionActiveModel {
            user_id: Set(user_id),
            workspace_id: Set(workspace.uuid.clone()),
            r#type: Set(PermissionType::Owner as i32),
            accepted: Set(true),
            ..Default::default()
        })
        .exec(trx)
        .await?;

        Ok(workspace)
    }

    // pub async fn create_normal_workspace(&self, user_id: i32) -> sqlx::Result<Workspace> {
    //     let mut trx = self.db.begin().await?;
    //     let workspace = Self::create_workspace(&mut trx, user_id, WorkspaceType::Normal).await?;

    //     trx.commit().await?;

    //     Ok(workspace)
    // }

    // pub async fn update_workspace(
    //     &self,
    //     workspace_id: String,
    //     data: UpdateWorkspace,
    // ) -> sqlx::Result<Option<Workspace>> {
    //     let update_workspace = format!(
    //         "UPDATE workspaces
    //             SET public = $1
    //         WHERE uuid = $2 AND type = {}
    //         RETURNING uuid as id, public, type, created_at;",
    //         WorkspaceType::Normal as i16
    //     );

    //     query_as::<_, Workspace>(&update_workspace)
    //         .bind(data.public)
    //         .bind(workspace_id)
    //         .fetch_optional(&self.db)
    //         .await
    // }

    // pub async fn delete_workspace(&self, workspace_id: String) -> sqlx::Result<bool> {
    //     let delete_permissions_workspace = format!(
    //         r#"DELETE FROM permissions CASCADE WHERE workspace_id = $1 AND (SELECT True FROM workspaces WHERE uuid = $1 AND type = {})"#,
    //         WorkspaceType::Normal as i16
    //     );

    //     query(&delete_permissions_workspace)
    //         .bind(workspace_id.clone())
    //         .execute(&self.db)
    //         .await
    //         .expect("delete table permissions failed");

    //     let delete_workspace = format!(
    //         r#"DELETE FROM workspaces CASCADE WHERE uuid = $1 AND type = {}"#,
    //         WorkspaceType::Normal as i16
    //     );

    //     query(&delete_workspace)
    //         .bind(workspace_id.clone())
    //         .execute(&self.db)
    //         .await
    //         .map(|q| q.rows_affected() != 0)
    // }

    // pub async fn get_user_workspaces(
    //     &self,
    //     user_id: i32,
    // ) -> sqlx::Result<Vec<WorkspaceWithPermission>> {
    //     let stmt = "SELECT
    //         workspaces.uuid AS id, workspaces.public, workspaces.created_at, workspaces.type,
    //         permissions.type as permission
    //     FROM permissions
    //     INNER JOIN workspaces
    //       ON permissions.workspace_id = workspaces.uuid
    //     WHERE user_id = $1 AND accepted = True";

    //     query_as::<_, WorkspaceWithPermission>(&stmt)
    //         .bind(user_id)
    //         .fetch_all(&self.db)
    //         .await
    // }

    // pub async fn get_workspace_members(&self, workspace_id: String) -> sqlx::Result<Vec<Member>> {
    //     let stmt = "SELECT
    //         permissions.id, permissions.type, permissions.user_email,
    //         permissions.accepted, permissions.created_at,
    //         users.id as user_id, users.name as user_name, users.email as user_table_email, users.avatar_url,
    //         users.created_at as user_created_at
    //     FROM permissions
    //     LEFT JOIN users
    //         ON users.id = permissions.user_id
    //     WHERE workspace_id = $1";

    //     query_as::<_, Member>(stmt)
    //         .bind(workspace_id)
    //         .fetch_all(&self.db)
    //         .await
    // }

    // pub async fn get_permission(
    //     &self,
    //     user_id: i32,
    //     workspace_id: String,
    // ) -> sqlx::Result<Option<PermissionType>> {
    //     let stmt = "SELECT type FROM permissions WHERE user_id = $1 AND workspace_id = $2";

    //     query_as::<_, PermissionQuery>(&stmt)
    //         .bind(user_id)
    //         .bind(workspace_id)
    //         .fetch_optional(&self.db)
    //         .await
    //         .map(|p| p.map(|p| p.type_))
    // }

    // pub async fn get_permission_by_permission_id(
    //     &self,
    //     user_id: i32,
    //     permission_id: i64,
    // ) -> sqlx::Result<Option<PermissionType>> {
    //     let stmt = "SELECT type FROM permissions
    //     WHERE
    //         user_id = $1
    //     AND
    //         workspace_id = (SELECT workspace_id FROM permissions WHERE permissions.id = $2)
    //     ";

    //     query_as::<_, PermissionQuery>(&stmt)
    //         .bind(user_id)
    //         .bind(permission_id)
    //         .fetch_optional(&self.db)
    //         .await
    //         .map(|p| p.map(|p| p.type_))
    // }

    // pub async fn can_read_workspace(
    //     &self,
    //     user_id: i32,
    //     workspace_id: String,
    // ) -> sqlx::Result<bool> {
    //     let stmt = "SELECT FROM permissions
    //         WHERE user_id = $1
    //             AND workspace_id = $2 AND accepted = True
    //             OR (SELECT True FROM workspaces WHERE uuid = $2 AND public = True)";

    //     query(&stmt)
    //         .bind(user_id)
    //         .bind(workspace_id)
    //         .fetch_optional(&self.db)
    //         .await
    //         .map(|p| p.is_some())
    // }

    // pub async fn is_public_workspace(&self, workspace_id: String) -> sqlx::Result<bool> {
    //     let stmt = "SELECT True FROM workspaces WHERE uuid = $1 AND public = True";

    //     query(&stmt)
    //         .bind(workspace_id)
    //         .fetch_optional(&self.db)
    //         .await
    //         .map(|p| p.is_some())
    // }

    // pub async fn create_permission(
    //     &self,
    //     email: &str,
    //     workspace_id: String,
    //     permission_type: PermissionType,
    // ) -> sqlx::Result<Option<(i64, UserCred)>> {
    //     let user = self.get_user_by_email(email).await?;

    //     let stmt = format!(
    //         "INSERT INTO permissions (user_id, user_email, workspace_id, type)
    //         SELECT $1, $2, $3, $4
    //         FROM workspaces
    //             WHERE workspaces.type = {} AND workspaces.uuid = $3
    //         ON CONFLICT DO NOTHING
    //         RETURNING id",
    //         WorkspaceType::Normal as i16
    //     );

    //     let query = query_as::<_, BigId>(&stmt);

    //     let (query, user) = match user {
    //         Some(user) => (
    //             query.bind(user.id).bind::<Option<String>>(None),
    //             UserCred::Registered(user),
    //         ),
    //         None => (
    //             query.bind::<Option<i32>>(None).bind(email),
    //             UserCred::UnRegistered {
    //                 email: email.to_owned(),
    //             },
    //         ),
    //     };

    //     let id = query
    //         .bind(workspace_id)
    //         .bind(permission_type as i16)
    //         .fetch_optional(&self.db)
    //         .await?;

    //     Ok(if let Some(id) = id {
    //         Some((id.id, user))
    //     } else {
    //         None
    //     })
    // }

    // pub async fn accept_permission(&self, permission_id: i64) -> sqlx::Result<Option<Permission>> {
    //     let stmt = "UPDATE permissions
    //             SET accepted = True
    //         WHERE id = $1
    //         RETURNING id, user_id, user_email, workspace_id, type, accepted, created_at";

    //     query_as::<_, Permission>(&stmt)
    //         .bind(permission_id)
    //         .fetch_optional(&self.db)
    //         .await
    // }

    // pub async fn delete_permission(&self, permission_id: i64) -> sqlx::Result<bool> {
    //     let stmt = "DELETE FROM permissions WHERE id = $1";

    //     query(&stmt)
    //         .bind(permission_id)
    //         .execute(&self.db)
    //         .await
    //         .map(|q| q.rows_affected() != 0)
    // }

    // pub async fn delete_permission_by_query(
    //     &self,
    //     user_id: i32,
    //     workspace_id: String,
    // ) -> sqlx::Result<bool> {
    //     let stmt = format!(
    //         "DELETE FROM permissions
    //         WHERE user_id = $1 AND workspace_id = $2 AND type != {}",
    //         PermissionType::Owner as i16
    //     );

    //     query(&stmt)
    //         .bind(user_id)
    //         .bind(workspace_id)
    //         .execute(&self.db)
    //         .await
    //         .map(|q| q.rows_affected() != 0)
    // }

    // pub async fn get_user_in_workspace_by_email(
    //     &self,
    //     workspace_id: String,
    //     email: &str,
    // ) -> sqlx::Result<UserInWorkspace> {
    //     let stmt = "SELECT
    //         id, name, email, avatar_url, token_nonce, created_at
    //     FROM users";

    //     let user = query_as::<_, User>(stmt)
    //         .bind(workspace_id.clone())
    //         .fetch_optional(&self.db)
    //         .await?;

    //     Ok(if let Some(user) = user {
    //         let stmt = "SELECT True FROM permissions WHERE workspace_id = $1 AND user_id = $2";

    //         let in_workspace = query(stmt)
    //             .bind(workspace_id)
    //             .bind(user.id)
    //             .fetch_optional(&self.db)
    //             .await?
    //             .is_some();

    //         UserInWorkspace {
    //             user: UserCred::Registered(user),
    //             in_workspace,
    //         }
    //     } else {
    //         let stmt = "SELECT True FROM permissions WHERE workspace_id = $1 AND user_email = $2";

    //         let in_workspace = query_as::<_, User>(stmt)
    //             .bind(workspace_id.clone())
    //             .bind(email)
    //             .fetch_optional(&self.db)
    //             .await?
    //             .is_some();

    //         UserInWorkspace {
    //             user: UserCred::UnRegistered {
    //                 email: email.to_string(),
    //             },
    //             in_workspace,
    //         }
    //     })
    // }
}

// #[cfg(test)]
// mod tests {
//     #[ignore = "need postgres instance"]
//     #[tokio::test]
//     async fn database_create_tables() -> anyhow::Result<()> {
//         use super::*;
//         // let check_table_exists =
//         // "IF EXISTS(SELECT 1 FROM sys.Tables WHERE  Name = N'users' AND Type = N'U')
//         //     RETURN True
//         // ELSE
//         //     RETURN False";
//         let db_context =
//             PostgreSQL::new("postgresql://jwst:jwst@localhost:5432/jwst".to_string()).await;
//         // clean db
//         let drop_statement = "
//         DROP TABLE IF EXISTS \"google_users\";
//         DROP TABLE IF EXISTS \"permissions\";
//         DROP TABLE IF EXISTS \"workspaces\";
//         DROP TABLE IF EXISTS \"users\";
//         ";
//         for line in drop_statement.split("\n") {
//             query(&line)
//                 .execute(&db_context.db)
//                 .await
//                 .expect("Drop all table in test failed");
//         }
//         db_context.init_db().await;
//         // start test
//         let (new_user, _) = db_context
//             .create_user(CreateUser {
//                 avatar_url: Some("xxx".to_string()),
//                 email: "xxx@xxx.xx".to_string(),
//                 name: "xxx".to_string(),
//                 password: "xxx".to_string(),
//             })
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(new_user.id, 1);
//         let (new_user2, _) = db_context
//             .create_user(CreateUser {
//                 avatar_url: Some("xxx".to_string()),
//                 email: "xxx2@xxx.xx".to_string(),
//                 name: "xxx".to_string(),
//                 password: "xxx".to_string(),
//             })
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(new_user2.id, 2);
//         let mut new_workspace = db_context
//             .create_normal_workspace(new_user.id)
//             .await
//             .unwrap();
//         let new_workspace2 = db_context
//             .create_normal_workspace(new_user2.id)
//             .await
//             .unwrap();

//         assert_eq!(new_workspace.id.len(), 36);
//         assert_eq!(new_workspace.public, false);
//         assert_eq!(new_workspace2.id.len(), 36);

//         let new_workspace1_clone = db_context
//             .get_workspace_by_id(new_workspace.id.clone())
//             .await
//             .unwrap()
//             .unwrap();

//         assert_eq!(new_user.id, new_workspace1_clone.owner.unwrap().id);
//         assert_eq!(new_workspace.id, new_workspace1_clone.workspace.id);
//         assert_eq!(
//             new_workspace.created_at,
//             new_workspace1_clone.workspace.created_at
//         );

//         assert_eq!(
//             new_workspace.id,
//             db_context
//                 .get_user_workspaces(new_user.id)
//                 .await
//                 .unwrap()
//                 // when create user, will auto create a private workspace, our created will be second one
//                 .get(1)
//                 .unwrap()
//                 .workspace
//                 .id
//         );

//         new_workspace = db_context
//             .update_workspace(new_workspace.id, UpdateWorkspace { public: true })
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(new_workspace.public, true);
//         Ok(())
//     }
// }
