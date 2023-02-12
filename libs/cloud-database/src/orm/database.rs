use super::{
    model::{
        CreateUser, GoogleClaims, Member, MemberResult, PermissionType, RefreshToken,
        UpdateWorkspace, User, UserCred, UserInWorkspace, UserLogin, UserWithNonce, Workspace,
        WorkspaceDetail, WorkspaceType, WorkspaceWithPermission,
    },
    *,
};
use affine_cloud_migration::{Expr, JoinType, Migrator, MigratorTrait, Query};
use sea_orm::{
    prelude::*, ConnectionTrait, Database, DatabaseTransaction, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

// #[derive(FromRow)]
// struct PermissionQuery {
//     #[sqlx(rename = "type")]
//     type_: PermissionType,
// }

pub struct CloudDatabase {
    pub pool: DatabaseConnection,
}

impl CloudDatabase {
    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self { pool })
    }

    // TODO UUID
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
        Permissions::find()
            .column_as(UsersColumn::Uuid, "id")
            .column(UsersColumn::Name)
            .column(UsersColumn::Email)
            .column(UsersColumn::AvatarUrl)
            .column(UsersColumn::CreatedAt)
            .join_rev(
                JoinType::InnerJoin,
                Users::belongs_to(Permissions)
                    .from(UsersColumn::Uuid)
                    .to(PermissionColumn::UserId)
                    .into(),
            )
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .filter(PermissionColumn::Type.eq(PermissionType::Owner as i16))
            .into_model::<UsersModel>()
            .one(&self.pool)
            .await
    }

    // todo uuid
    pub async fn user_login(&self, login: UserLogin) -> Result<Option<UsersModel>, DbErr> {
        Users::find()
            .filter(UsersColumn::Email.eq(login.email))
            .filter(UsersColumn::Password.eq(login.password))
            .one(&self.pool)
            .await
    }

    // todo uuid
    pub async fn refresh_token(&self, token: RefreshToken) -> Result<Option<UsersModel>, DbErr> {
        Users::find()
            .filter(UsersColumn::Uuid.eq(token.user_id))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
    }

    pub async fn verify_refresh_token(&self, token: &RefreshToken) -> Result<bool, DbErr> {
        Users::find()
            .column(UsersColumn::Id)
            .filter(UsersColumn::Uuid.eq(token.user_id.clone()))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
            .map(|r| r.is_some())
    }

    pub async fn update_cred(
        trx: &DatabaseTransaction,
        user_id: String,
        user_email: &str,
    ) -> Result<(), DbErr> {
        Permissions::update(PermissionActiveModel {
            user_id: Set(Some(user_id)),
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
    ) -> Result<Option<(UsersModel, Workspace)>, DbErr> {
        let trx = self.pool.begin().await?;

        let uuid = Uuid::new_v4().to_string();
        let Ok(user) = Users::insert(UsersActiveModel {
            uuid: Set(uuid.clone()),
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
            .create_workspace(&trx, uuid.clone(), WorkspaceType::Private)
            .await?;

        Self::update_cred(&trx, uuid, &user.email).await?;

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

        let member_count = Permissions::find()
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .filter(PermissionColumn::Accepted.eq(true))
            .count(&self.pool)
            .await?;

        Ok(Some(WorkspaceDetail {
            owner: Some(User {
                id: owner.uuid,
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
        user_id: String,
        ws_type: WorkspaceType,
    ) -> Result<Workspace, DbErr> {
        let uuid = Uuid::new_v4();
        let workspace_id = uuid.to_string().replace('-', "_");

        let workspace = Workspaces::insert(WorkspacesActiveModel {
            public: Set(false),
            r#type: Set(ws_type as i32),
            uuid: Set(workspace_id),
            ..Default::default()
        })
        .exec_with_returning(trx)
        .await
        .map(|ws| Workspace {
            id: ws.uuid,
            public: ws.public,
            r#type: ws.r#type.into(),
            created_at: ws.created_at.naive_local(),
        })?;

        Permissions::insert(PermissionActiveModel {
            user_id: Set(Some(user_id)),
            workspace_id: Set(workspace.id.clone()),
            r#type: Set(PermissionType::Owner as i32),
            accepted: Set(true),
            ..Default::default()
        })
        .exec(trx)
        .await?;

        Ok(workspace)
    }

    pub async fn create_normal_workspace(&self, user_id: String) -> Result<Workspace, DbErr> {
        let trx = self.pool.begin().await?;
        let workspace = self
            .create_workspace(&trx, user_id, WorkspaceType::Normal)
            .await?;

        trx.commit().await?;

        Ok(workspace)
    }

    pub async fn update_workspace(
        &self,
        workspace_id: String,
        data: UpdateWorkspace,
    ) -> Result<Option<Workspace>, DbErr> {
        let model = Workspaces::find()
            .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .one(&self.pool)
            .await?;
        if model.is_none() {
            return Ok(None);
        }

        let workspace = Workspaces::update(WorkspacesActiveModel {
            public: Set(data.public),
            ..Default::default()
        })
        .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
        .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
        .exec(&self.pool)
        .await
        .map(|ws| Workspace {
            id: ws.uuid,
            public: ws.public,
            r#type: ws.r#type.into(),
            created_at: ws.created_at.naive_local(),
        })?;
        Ok(Some(workspace))
    }

    pub async fn delete_workspace(&self, workspace_id: String) -> Result<bool, DbErr> {
        let trx = self.pool.begin().await?;

        Permissions::delete_many()
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .filter(Expr::exists(
                Query::select()
                    .from(Workspaces)
                    .and_where(
                        Expr::tbl(Workspaces, WorkspacesColumn::Uuid).eq(workspace_id.clone()),
                    )
                    .and_where(
                        Expr::tbl(Workspaces, WorkspacesColumn::Type)
                            .eq(WorkspaceType::Normal as i32),
                    )
                    .take(),
            ))
            .exec(&trx)
            .await?;

        Workspaces::delete_many()
            .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .exec(&trx)
            .await
            .map(|r| r.rows_affected > 0)
    }

    pub async fn get_user_workspaces(
        &self,
        user_id: String,
    ) -> Result<Vec<WorkspaceWithPermission>, DbErr> {
        Permissions::find()
            .column_as(WorkspacesColumn::Uuid, "id")
            .column_as(WorkspacesColumn::Public, "public")
            .column_as(WorkspacesColumn::CreatedAt, "created_at")
            .column_as(WorkspacesColumn::Type, "type")
            .column_as(PermissionColumn::Type, "permission")
            .join_rev(
                JoinType::InnerJoin,
                Workspaces::belongs_to(Permissions)
                    .from(WorkspacesColumn::Uuid)
                    .to(PermissionColumn::WorkspaceId)
                    .into(),
            )
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::Accepted.eq(true))
            .into_model::<WorkspaceWithPermission>()
            .all(&self.pool)
            .await
    }

    pub async fn get_workspace_members(&self, workspace_id: String) -> Result<Vec<Member>, DbErr> {
        Permissions::find()
            .column_as(PermissionColumn::Id, "id")
            .column_as(PermissionColumn::Type, "type")
            .column_as(PermissionColumn::Accepted, "accepted")
            .column_as(PermissionColumn::CreatedAt, "created_at")
            .column_as(UsersColumn::Uuid, "user_id")
            .column_as(UsersColumn::Name, "user_name")
            .column_as(UsersColumn::Email, "user_email")
            .column_as(UsersColumn::AvatarUrl, "user_avatar_url")
            .column_as(UsersColumn::CreatedAt, "user_created_at")
            .join_rev(
                JoinType::LeftJoin,
                Users::belongs_to(Permissions)
                    .from(UsersColumn::Uuid)
                    .to(PermissionColumn::UserId)
                    .into(),
            )
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .into_model::<MemberResult>()
            .all(&self.pool)
            .await
            .map(|m| m.iter().map(|m| m.into()).collect())
    }

    pub async fn get_permission(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<Option<PermissionType>, DbErr> {
        Permissions::find()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .one(&self.pool)
            .await
            .map(|p| p.map(|p| p.r#type.into()))
    }

    pub async fn get_permission_by_permission_id(
        &self,
        user_id: String,
        permission_id: i32,
    ) -> Result<Option<PermissionType>, DbErr> {
        Permissions::find()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(
                PermissionColumn::WorkspaceId.in_subquery(
                    Query::select()
                        .from(Permissions)
                        .column(PermissionColumn::WorkspaceId)
                        .and_where(Expr::tbl(Permissions, PermissionColumn::Id).eq(permission_id))
                        .take(),
                ),
            )
            .one(&self.pool)
            .await
            .map(|p| p.map(|p| p.r#type.into()))
    }

    pub async fn can_read_workspace(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<bool, DbErr> {
        Permissions::find()
            .filter(
                PermissionColumn::UserId
                    .eq(user_id)
                    .and(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
                    .and(PermissionColumn::Accepted.eq(true))
                    .or(Expr::exists(
                        Query::select()
                            .from(Workspaces)
                            .and_where(
                                Expr::tbl(Workspaces, WorkspacesColumn::Uuid)
                                    .eq(workspace_id.clone()),
                            )
                            .and_where(Expr::tbl(Workspaces, WorkspacesColumn::Public).eq(true))
                            .take(),
                    )),
            )
            .one(&self.pool)
            .await
            .map(|p| p.is_some())
    }

    pub async fn is_public_workspace(&self, workspace_id: String) -> Result<bool, DbErr> {
        Workspaces::find()
            .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Public.eq(true))
            .one(&self.pool)
            .await
            .map(|p| p.is_some())
    }

    pub async fn create_permission(
        &self,
        email: &str,
        workspace_id: String,
        permission_type: PermissionType,
    ) -> Result<Option<(i32, UserCred)>, DbErr> {
        let workspace = Workspaces::find()
            .filter(WorkspacesColumn::Uuid.eq(workspace_id.clone()))
            // todo: check if this is correct
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .one(&self.pool)
            .await?;
        if workspace.is_none() {
            return Ok(None);
        }

        let user = self.get_user_by_email(email).await?;
        let id = Permissions::insert(PermissionActiveModel {
            user_id: Set(user.clone().and_then(|u| Some(u.uuid))),
            user_email: Set(user.clone().and(None).or(Some(email.to_string()))),
            workspace_id: Set(workspace_id),
            r#type: Set(permission_type as i32),
            ..Default::default()
        })
        .exec_with_returning(&self.pool)
        .await
        .map(|p| p.id)?;

        let user = match user {
            Some(user) => UserCred::Registered(User {
                id: user.uuid,
                name: user.name,
                email: user.email,
                avatar_url: user.avatar_url,
                created_at: user.created_at.unwrap_or_default().naive_local(),
            }),
            None => UserCred::UnRegistered {
                email: email.to_owned(),
            },
        };

        Ok(Some((id, user)))
    }

    pub async fn accept_permission(&self, permission_id: i64) -> Result<Option<Permission>, DbErr> {
        let p = Permissions::find()
            .filter(PermissionColumn::Id.eq(permission_id))
            .one(&self.pool)
            .await?;

        if p.is_none() {
            return Ok(None);
        }

        Ok(Some(
            Permissions::update(PermissionActiveModel {
                accepted: Set(true),
                ..Default::default()
            })
            .filter(PermissionColumn::Id.eq(permission_id))
            .exec(&self.pool)
            .await
            .map(|op| Permission {
                id: op.id,
                type_: op.r#type.into(),
                workspace_id: op.workspace_id,
                user_id: op.user_id,
                user_email: op.user_email,
                accepted: op.accepted,
                created_at: op.created_at.naive_local(),
            })?,
        ))
    }

    pub async fn delete_permission(&self, permission_id: i32) -> Result<bool, DbErr> {
        let trx = self.pool.begin().await?;

        Permissions::delete_many()
            .filter(PermissionColumn::Id.eq(permission_id))
            .exec(&trx)
            .await
            .map(|q| q.rows_affected > 0)
    }

    pub async fn delete_permission_by_query(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<bool, DbErr> {
        let trx = self.pool.begin().await?;

        Permissions::delete_many()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .exec(&trx)
            .await
            .map(|q| q.rows_affected > 0)
    }

    pub async fn get_user_in_workspace_by_email(
        &self,
        workspace_id: String,
        email: &str,
    ) -> Result<UserInWorkspace, DbErr> {
        let user: Option<UsersModel> = Users::find()
            .filter(UsersColumn::Email.eq(email))
            .one(&self.pool)
            .await?;

        Ok(if let Some(user) = user {
            let in_workspace = Permissions::find()
                .filter(PermissionColumn::UserId.eq(user.id))
                .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
                .one(&self.pool)
                .await
                .map(|p| p.is_some())?;

            UserInWorkspace {
                user: UserCred::Registered(User {
                    id: user.uuid,
                    name: user.name,
                    email: user.email,
                    avatar_url: user.avatar_url,
                    created_at: user.created_at.unwrap_or_default().naive_local(),
                }),
                in_workspace,
            }
        } else {
            let in_workspace = Permissions::find()
                .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
                .filter(PermissionColumn::UserEmail.eq(email))
                .one(&self.pool)
                .await
                .map(|p| p.is_some())?;

            UserInWorkspace {
                user: UserCred::UnRegistered {
                    email: email.to_string(),
                },
                in_workspace,
            }
        })
    }

    pub async fn google_user_login(&self, claims: &GoogleClaims) -> Result<UsersModel, DbErr> {
        let google_user: Option<GoogleUsersModel> = GoogleUsers::find()
            .filter(GoogleUsersColumn::UserId.eq(claims.user_id.clone()))
            .one(&self.pool)
            .await?;
        if let Some(google_user) = google_user {
            let user = Users::update(UsersActiveModel {
                name: Set(claims.name.clone()),
                email: Set(claims.email.clone()),
                avatar_url: Set(Some(claims.picture.clone())),
                ..Default::default()
            })
            .filter(UsersColumn::Uuid.eq(google_user.user_id))
            .exec(&self.pool)
            .await?;
            // let user_with_nonce = UserWithNonce {
            //     user: User {
            //         id: user.uuid,
            //         name: user.name,
            //         email: user.email,
            //         avatar_url: user.avatar_url,
            //         created_at: user.created_at.unwrap_or_default().naive_local(),
            //     },
            //     token_nonce: user.token_nonce.unwrap_or_default(),
            // };
            // Ok(user_with_nonce)
            Ok(user)
        } else {
            let uuid = uuid::Uuid::new_v4().to_string();
            let user = Users::insert(UsersActiveModel {
                uuid: Set(uuid),
                name: Set(claims.name.clone()),
                email: Set(claims.email.clone()),
                avatar_url: Set(Some(claims.picture.clone())),
                ..Default::default()
            })
            .exec_with_returning(&self.pool)
            .await?;
            GoogleUsers::insert(GoogleUsersActiveModel {
                user_id: Set(user.uuid.clone()),
                google_id: Set(claims.user_id.clone()),
                ..Default::default()
            })
            .exec_with_returning(&self.pool)
            .await?;
            // let user_with_nonce = UserWithNonce {
            //     user: User {
            //         id: user.uuid,
            //         name: user.name,
            //         email: user.email,
            //         avatar_url: user.avatar_url,
            //         created_at: user.created_at.unwrap_or_default().naive_local(),
            //     },
            //     token_nonce: user.token_nonce.unwrap_or_default(),
            // };
            // Ok(user_with_nonce)
            Ok(user)
        }
    }
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
