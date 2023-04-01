use super::{
    model::{
        CreateUser, FirebaseClaims, Member, MemberResult, PermissionType, RefreshToken,
        UpdateWorkspace, User, UserCred, UserInWorkspace, UserLogin, Workspace, WorkspaceDetail,
        WorkspaceType, WorkspaceWithPermission,
    },
    *,
};
use affine_cloud_migration::{Expr, JoinType, Migrator, MigratorTrait, Query};
use jwst_logger::{info, instrument, tracing};
use nanoid::nanoid;
use sea_orm::{
    prelude::*, ConnectionTrait, Database, DatabaseTransaction, QuerySelect, Set, TransactionTrait,
};

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

    #[instrument(skip(self))]
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UsersModel>, DbErr> {
        info!("database get_user_by_email enter");
        Users::find()
            .filter(UsersColumn::Email.eq(email))
            .one(&self.pool)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_workspace_owner(
        &self,
        workspace_id: String,
    ) -> Result<Option<UsersModel>, DbErr> {
        info!("database get_workspace_owner enter");
        Permissions::find()
            .column(UsersColumn::Id)
            .column(UsersColumn::Name)
            .column(UsersColumn::Email)
            .column(UsersColumn::AvatarUrl)
            .column(UsersColumn::CreatedAt)
            .column(UsersColumn::Password)
            .column(UsersColumn::TokenNonce)
            .join_rev(
                JoinType::InnerJoin,
                Users::belongs_to(Permissions)
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

    #[instrument(skip(self))]
    pub async fn user_login(&self, login: UserLogin) -> Result<Option<UsersModel>, DbErr> {
        info!("database user_login enter");
        Users::find()
            .filter(UsersColumn::Email.eq(login.email))
            .filter(UsersColumn::Password.eq(login.password))
            .one(&self.pool)
            .await
    }

    #[instrument(skip(self, token))]
    pub async fn refresh_token(&self, token: RefreshToken) -> Result<Option<UsersModel>, DbErr> {
        info!("database refresh_token enter");
        Users::find()
            .filter(UsersColumn::Id.eq(token.user_id))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
    }

    #[instrument(skip(self, token))]
    pub async fn verify_refresh_token(&self, token: &RefreshToken) -> Result<bool, DbErr> {
        info!("database verify_refresh_token enter");
        Users::find()
            .column(UsersColumn::Id)
            .filter(UsersColumn::Id.eq(token.user_id.clone()))
            .filter(UsersColumn::TokenNonce.eq(token.token_nonce))
            .one(&self.pool)
            .await
            .map(|r| r.is_some())
    }

    #[instrument(skip(trx))]
    pub async fn update_cred(
        trx: &DatabaseTransaction,
        user_id: String,
        user_email: &str,
    ) -> Result<Option<()>, DbErr> {
        info!("database update_cred enter");
        let model = Permissions::find()
            .filter(PermissionColumn::UserEmail.eq(user_email))
            .one(trx)
            .await?;
        if model.is_none() {
            return Ok(None);
        }

        let id = model.unwrap().id;
        Permissions::update(PermissionActiveModel {
            id: Set(id.clone()),
            user_id: Set(Some(user_id)),
            user_email: Set(None),
            ..Default::default()
        })
        .filter(PermissionColumn::Id.eq(id))
        .exec(trx)
        .await
        .map(|_| Some(()))
    }

    #[instrument(skip(self))]
    pub async fn create_user(&self, user: CreateUser) -> Result<UsersModel, DbErr> {
        info!("database create_user enter");
        let trx = self.pool.begin().await?;

        let id = nanoid!();
        let Ok(user) = Users::insert(UsersActiveModel {
            id: Set(id.clone()),
            name: Set(user.name),
            password: Set(Some(user.password)),
            email: Set(user.email),
            avatar_url: Set(user.avatar_url),
            ..Default::default()
        })
        .exec_with_returning(&trx)
        .await else {
            trx.rollback().await?;
            return Err(DbErr::RecordNotUpdated)
        };

        Self::update_cred(&trx, id, &user.email).await?;

        trx.commit().await?;

        Ok(user)
    }

    #[instrument(skip(self))]
    pub async fn get_workspace_by_id(
        &self,
        workspace_id: String,
    ) -> Result<Option<WorkspaceDetail>, DbErr> {
        info!("database get_workspace_by_id enter");
        let workspace = Workspaces::find()
            .filter(WorkspacesColumn::Id.eq(workspace_id.clone()))
            .one(&self.pool)
            .await?;

        let workspace = match workspace {
            Some(workspace) if workspace.r#type == WorkspaceType::Private as i16 => {
                return Ok(Some(WorkspaceDetail {
                    owner: None,
                    member_count: 0,
                    workspace: Workspace {
                        id: workspace.id.clone(),
                        public: workspace.public,
                        r#type: workspace.r#type.into(),
                        created_at: workspace.created_at.unwrap_or_default().naive_local(),
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
                id: owner.id,
                name: owner.name,
                email: owner.email,
                avatar_url: owner.avatar_url,
                created_at: owner.created_at.unwrap_or_default().naive_local(),
            }),
            member_count,
            workspace: Workspace {
                id: workspace.id.clone(),
                public: workspace.public,
                r#type: workspace.r#type.into(),
                created_at: workspace.created_at.unwrap_or_default().naive_local(),
            },
        }))
    }

    #[instrument(skip(self, trx))]
    pub async fn create_workspace<C: ConnectionTrait>(
        &self,
        trx: &C,
        user_id: String,
        ws_type: WorkspaceType,
    ) -> Result<Workspace, DbErr> {
        info!("database create_workspace enter");
        let id = nanoid!();
        let workspace = Workspaces::insert(WorkspacesActiveModel {
            id: Set(id),
            public: Set(false),
            r#type: Set(ws_type as i16),
            ..Default::default()
        })
        .exec_with_returning(trx)
        .await
        .map(|ws| Workspace {
            id: ws.id,
            public: ws.public,
            r#type: ws.r#type.into(),
            created_at: ws.created_at.unwrap_or_default().naive_local(),
        })?;

        let permissions_id = nanoid!();
        Permissions::insert(PermissionActiveModel {
            id: Set(permissions_id),
            user_id: Set(Some(user_id)),
            workspace_id: Set(workspace.id.clone()),
            r#type: Set(PermissionType::Owner as i16),
            accepted: Set(true),
            ..Default::default()
        })
        .exec(trx)
        .await?;

        Ok(workspace)
    }

    #[instrument(skip(self))]
    pub async fn create_normal_workspace(&self, user_id: String) -> Result<Workspace, DbErr> {
        info!("database create_normal_workspace enter");
        let trx = self.pool.begin().await?;
        let workspace = self
            .create_workspace(&trx, user_id, WorkspaceType::Normal)
            .await?;

        trx.commit().await?;

        Ok(workspace)
    }

    #[instrument(skip(self))]
    pub async fn update_workspace(
        &self,
        workspace_id: String,
        data: UpdateWorkspace,
    ) -> Result<Option<Workspace>, DbErr> {
        info!("database update_workspace enter");
        let model = Workspaces::find()
            .filter(WorkspacesColumn::Id.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .one(&self.pool)
            .await?;
        if model.is_none() {
            return Ok(None);
        }

        let id = model.unwrap().id;
        let workspace = Workspaces::update(WorkspacesActiveModel {
            id: Set(id.clone()),
            public: Set(data.public),
            ..Default::default()
        })
        .filter(WorkspacesColumn::Id.eq(id))
        .exec(&self.pool)
        .await
        .map(|ws| Workspace {
            id: ws.id,
            public: ws.public,
            r#type: ws.r#type.into(),
            created_at: ws.created_at.unwrap_or_default().naive_local(),
        })?;
        Ok(Some(workspace))
    }

    #[instrument(skip(self))]
    pub async fn delete_workspace(&self, workspace_id: String) -> Result<bool, DbErr> {
        info!("database delete_workspace enter");
        let trx = self.pool.begin().await?;

        Permissions::delete_many()
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .filter(Expr::exists(
                Query::select()
                    .from(Workspaces)
                    .column(WorkspacesColumn::Id)
                    .and_where(
                        Expr::col((Workspaces, WorkspacesColumn::Id)).eq(workspace_id.clone()),
                    )
                    .and_where(
                        Expr::col((Workspaces, WorkspacesColumn::Type))
                            .eq(WorkspaceType::Normal as i32),
                    )
                    .limit(1)
                    .take(),
            ))
            .exec(&trx)
            .await?;

        let success = Workspaces::delete_many()
            .filter(WorkspacesColumn::Id.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .exec(&trx)
            .await
            .map(|r| r.rows_affected > 0)?;

        trx.commit().await?;
        Ok(success)
    }

    #[instrument(skip(self))]
    pub async fn get_user_workspaces(
        &self,
        user_id: String,
    ) -> Result<Vec<WorkspaceWithPermission>, DbErr> {
        info!("database get_user_workspaces enter");
        Permissions::find()
            .column_as(WorkspacesColumn::Id, "id")
            .column_as(WorkspacesColumn::Public, "public")
            .column_as(WorkspacesColumn::CreatedAt, "created_at")
            .column_as(WorkspacesColumn::Type, "type")
            .column_as(PermissionColumn::Type, "permission")
            .join_rev(
                JoinType::InnerJoin,
                Workspaces::belongs_to(Permissions)
                    .from(WorkspacesColumn::Id)
                    .to(PermissionColumn::WorkspaceId)
                    .into(),
            )
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::Accepted.eq(true))
            .into_model::<WorkspaceWithPermission>()
            .all(&self.pool)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_workspace_members(&self, workspace_id: String) -> Result<Vec<Member>, DbErr> {
        info!("database get_workspace_members enter");
        Permissions::find()
            .column_as(PermissionColumn::Id, "id")
            .column_as(PermissionColumn::Type, "type")
            .column_as(PermissionColumn::UserEmail, "user_email")
            .column_as(PermissionColumn::Accepted, "accepted")
            .column_as(PermissionColumn::CreatedAt, "created_at")
            .column_as(UsersColumn::Id, "user_id")
            .column_as(UsersColumn::Name, "user_name")
            .column_as(UsersColumn::Email, "user_table_email")
            .column_as(UsersColumn::AvatarUrl, "user_avatar_url")
            .column_as(UsersColumn::CreatedAt, "user_created_at")
            .join_rev(
                JoinType::LeftJoin,
                Users::belongs_to(Permissions)
                    .from(UsersColumn::Id)
                    .to(PermissionColumn::UserId)
                    .into(),
            )
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .into_model::<MemberResult>()
            .all(&self.pool)
            .await
            .map(|m| m.iter().map(|m| m.into()).collect())
    }

    #[instrument(skip(self))]
    pub async fn get_permission(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<Option<PermissionType>, DbErr> {
        info!("database get_permission enter");
        Permissions::find()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
            .one(&self.pool)
            .await
            .map(|p| p.map(|p| p.r#type.into()))
    }

    #[instrument(skip(self))]
    pub async fn get_permission_by_permission_id(
        &self,
        user_id: String,
        permission_id: String,
    ) -> Result<Option<PermissionType>, DbErr> {
        info!("database get_permission_by_permission_id enter");
        Permissions::find()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(
                PermissionColumn::WorkspaceId.in_subquery(
                    Query::select()
                        .from(Permissions)
                        .column(PermissionColumn::WorkspaceId)
                        .and_where(Expr::col((Permissions, PermissionColumn::Id)).eq(permission_id))
                        .take(),
                ),
            )
            .one(&self.pool)
            .await
            .map(|p| p.map(|p| p.r#type.into()))
    }

    #[instrument(skip(self))]
    pub async fn get_permission_by_id(
        &self,
        permission_id: String,
    ) -> Result<Option<PermissionModel>, DbErr> {
        info!("database get_permission_by_id enter");
        Permissions::find()
            .filter(PermissionColumn::Id.eq(permission_id))
            .one(&self.pool)
            .await
    }

    #[instrument(skip(self))]
    pub async fn can_read_workspace(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<bool, DbErr> {
        info!("database can_read_workspace enter");
        Permissions::find()
            .filter(
                PermissionColumn::UserId
                    .eq(user_id)
                    .and(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
                    .and(PermissionColumn::Accepted.eq(true))
                    .or(Expr::exists(
                        Query::select()
                            .from(Workspaces)
                            .column(WorkspacesColumn::Id)
                            .and_where(
                                Expr::col((Workspaces, WorkspacesColumn::Id))
                                    .eq(workspace_id.clone()),
                            )
                            .and_where(Expr::col((Workspaces, WorkspacesColumn::Public)).eq(true))
                            .limit(1)
                            .take(),
                    )),
            )
            .one(&self.pool)
            .await
            .map(|p| p.is_some())
    }

    #[instrument(skip(self))]
    pub async fn is_public_workspace(&self, workspace_id: String) -> Result<bool, DbErr> {
        info!("database is_public_workspace enter");
        Workspaces::find()
            .filter(WorkspacesColumn::Id.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Public.eq(true))
            .one(&self.pool)
            .await
            .map(|p| p.is_some())
    }

    #[instrument(skip(self))]
    pub async fn create_permission(
        &self,
        email: &str,
        workspace_id: String,
        permission_type: PermissionType,
    ) -> Result<Option<(String, UserCred)>, DbErr> {
        info!("database create_permission enter");
        let workspace = Workspaces::find()
            .filter(WorkspacesColumn::Id.eq(workspace_id.clone()))
            .filter(WorkspacesColumn::Type.eq(WorkspaceType::Normal as i32))
            .one(&self.pool)
            .await?;
        if workspace.is_none() {
            return Ok(None);
        }

        let user = self.get_user_by_email(email).await?;
        let id = nanoid!();
        Permissions::insert(PermissionActiveModel {
            id: Set(id.clone()),
            user_id: Set(user.clone().map(|u| u.id)),
            user_email: Set(user.clone().and(None).or(Some(email.to_string()))),
            workspace_id: Set(workspace_id),
            r#type: Set(permission_type as i16),
            ..Default::default()
        })
        .exec(&self.pool)
        .await?;

        let user = match user {
            Some(user) => UserCred::Registered(User {
                id: user.id,
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

    #[instrument(skip(self))]
    pub async fn accept_permission(
        &self,
        permission_id: String,
    ) -> Result<Option<Permission>, DbErr> {
        info!("database accept_permission enter");
        let p = Permissions::find()
            .filter(PermissionColumn::Id.eq(permission_id.clone()))
            .one(&self.pool)
            .await?;

        if p.is_none() {
            return Ok(None);
        }

        Ok(Some(
            Permissions::update(PermissionActiveModel {
                id: Set(permission_id.clone()),
                accepted: Set(true),
                ..Default::default()
            })
            .filter(PermissionColumn::Id.eq(permission_id))
            .exec(&self.pool)
            .await
            .map(|op| Permission {
                id: op.id,
                r#type: op.r#type.into(),
                workspace_id: op.workspace_id,
                user_id: op.user_id,
                user_email: op.user_email,
                accepted: op.accepted,
                created_at: op.created_at.unwrap_or_default().naive_local(),
            })?,
        ))
    }

    #[instrument(skip(self))]
    pub async fn delete_permission(&self, permission_id: String) -> Result<bool, DbErr> {
        info!("database delete_permission enter");
        Permissions::delete_many()
            .filter(PermissionColumn::Id.eq(permission_id))
            .exec(&self.pool)
            .await
            .map(|q| q.rows_affected > 0)
    }

    #[instrument(skip(self))]
    pub async fn delete_permission_by_query(
        &self,
        user_id: String,
        workspace_id: String,
    ) -> Result<bool, DbErr> {
        info!("database delete_permission_by_query enter");
        Permissions::delete_many()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::WorkspaceId.eq(workspace_id.clone()))
            .exec(&self.pool)
            .await
            .map(|q| q.rows_affected > 0)
    }

    #[instrument(skip(self))]
    pub async fn get_user_in_workspace_by_email(
        &self,
        workspace_id: String,
        email: &str,
    ) -> Result<UserInWorkspace, DbErr> {
        info!("database get_user_in_workspace_by_email enter");
        let user: Option<UsersModel> = Users::find()
            .filter(UsersColumn::Email.eq(email))
            .one(&self.pool)
            .await?;

        Ok(if let Some(user) = user {
            let in_workspace = Permissions::find()
                .filter(PermissionColumn::UserId.eq(user.id.clone()))
                .filter(PermissionColumn::WorkspaceId.eq(workspace_id))
                .one(&self.pool)
                .await
                .map(|p| p.is_some())?;

            UserInWorkspace {
                user: UserCred::Registered(User {
                    id: user.id,
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

    #[instrument(skip(self))]
    pub async fn firebase_user_login(&self, claims: &FirebaseClaims) -> Result<UsersModel, DbErr> {
        info!("database firebase_user_login enter");
        let firebase_user: Option<GoogleUsersModel> = GoogleUsers::find()
            .filter(GoogleUsersColumn::GoogleId.eq(claims.user_id.clone()))
            .one(&self.pool)
            .await?;

        if let Some(user_info) = &claims.user_info {
            if let Some(firebase_user) = firebase_user {
                let id = Users::find()
                    .filter(UsersColumn::Id.eq(firebase_user.user_id.clone()))
                    .one(&self.pool)
                    .await?
                    .ok_or_else(|| DbErr::RecordNotFound(firebase_user.user_id.clone()))?
                    .id;

                let user = Users::update(UsersActiveModel {
                    id: Set(id.clone()),
                    name: Set(user_info.name.clone().unwrap_or("Uname".into())),
                    email: Set(user_info.email.clone()),
                    avatar_url: Set(user_info.picture.clone()),
                    ..Default::default()
                })
                .filter(UsersColumn::Id.eq(id))
                .exec(&self.pool)
                .await?;
                Ok(user)
            } else {
                let trx = self.pool.begin().await?;
                let id = nanoid!();
                let user = Users::insert(UsersActiveModel {
                    id: Set(id),
                    name: Set(user_info.name.clone().unwrap_or("Uname".into())),
                    email: Set(user_info.email.clone()),
                    avatar_url: Set(user_info.picture.clone()),
                    ..Default::default()
                })
                .exec_with_returning(&trx)
                .await?;
                let google_user_id = nanoid!();
                GoogleUsers::insert(GoogleUsersActiveModel {
                    id: Set(google_user_id),
                    user_id: Set(user.id.clone()),
                    google_id: Set(claims.user_id.clone()),
                })
                .exec_with_returning(&trx)
                .await?;
                Permissions::update_many()
                    .set(PermissionActiveModel {
                        user_id: Set(Some(user.id.clone())),
                        ..Default::default()
                    })
                    .filter(PermissionColumn::UserEmail.eq(user_info.email.clone()))
                    .exec(&trx)
                    .await?;
                trx.commit().await?;
                Ok(user)
            }
        } else {
            Err(DbErr::RecordNotInserted)
        }
    }

    #[instrument(skip(self))]
    pub async fn get_user_owner_workspaces(&self, user_id: String) -> Result<Vec<String>, DbErr> {
        info!("database get_user_owner_workspaces enter");
        Permissions::find()
            .filter(PermissionColumn::UserId.eq(user_id))
            .filter(PermissionColumn::Type.eq(PermissionType::Owner as i16))
            .all(&self.pool)
            .await
            .map(|m| m.iter().map(|m| m.workspace_id.clone()).collect())
    }
}

#[cfg(test)]
mod test {

    #[tokio::test]
    async fn database_create_tables() -> anyhow::Result<()> {
        use super::*;
        let pool = CloudDatabase::init_pool("sqlite::memory:").await?;
        // start test
        let new_user = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();
        let new_workspace = pool
            .create_normal_workspace(new_user.id.clone())
            .await
            .unwrap();
        assert_eq!(new_workspace.public, false);

        let new_workspace1_clone = pool
            .get_workspace_by_id(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(new_user.id, new_workspace1_clone.owner.unwrap().id);
        assert_eq!(new_workspace.id, new_workspace1_clone.workspace.id);
        assert_eq!(
            new_workspace.created_at,
            new_workspace1_clone.workspace.created_at
        );

        assert_eq!(
            new_workspace.id,
            pool.get_user_workspaces(new_user.id)
                .await
                .unwrap()
                .get(0)
                .unwrap()
                .id
        );

        Ok(())
    }

    #[tokio::test]
    async fn database_update_tables() -> anyhow::Result<()> {
        use super::*;
        let pool = CloudDatabase::init_pool("sqlite::memory:").await?;
        // start test
        let new_user = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();

        let mut new_workspace = pool
            .create_normal_workspace(new_user.id.clone())
            .await
            .unwrap();
        let is_published = pool
            .is_public_workspace(new_workspace.id.clone())
            .await
            .unwrap();
        let workspace_owner = pool
            .get_workspace_owner(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace_owner.id, new_user.id);
        assert!(!new_workspace.public);
        assert!(!is_published);
        new_workspace = pool
            .update_workspace(new_workspace.id.clone(), UpdateWorkspace { public: true })
            .await
            .unwrap()
            .unwrap();
        let is_published = pool
            .is_public_workspace(new_workspace.id.clone())
            .await
            .unwrap();
        assert!(new_workspace.public);
        assert!(is_published);

        Ok(())
    }

    #[tokio::test]
    async fn database_delete_tables() -> anyhow::Result<()> {
        use super::*;
        let pool = CloudDatabase::init_pool("sqlite::memory:").await?;
        // start test
        let new_user = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();

        let new_workspace = pool
            .create_normal_workspace(new_user.id.clone())
            .await
            .unwrap();

        let is_deleted = pool
            .delete_workspace(new_workspace.id.clone())
            .await
            .unwrap();
        assert_eq!(is_deleted, true);

        Ok(())
    }

    #[tokio::test]
    async fn database_permission() -> anyhow::Result<()> {
        use super::*;
        let pool = CloudDatabase::init_pool("sqlite::memory:").await?;
        // start test
        let new_user = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();
        let new_user2 = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx2@xxx.xx".to_string(),
                name: "xxx2".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();
        let new_user3 = pool
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx3@xxx.xx".to_string(),
                name: "xxx3".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();

        let new_workspace = pool
            .create_normal_workspace(new_user.id.clone())
            .await
            .unwrap();

        let workspace_owner = pool
            .get_workspace_owner(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace_owner.id, new_user.id);
        //Create permission
        let new_permission = pool
            .create_permission(
                &new_user2.email.clone(),
                new_workspace.id.clone(),
                PermissionType::Admin,
            )
            .await
            .unwrap()
            .unwrap();

        //accept permission
        let accept_permission = pool
            .accept_permission(new_permission.0.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            accept_permission.workspace_id.clone(),
            new_workspace.id.clone()
        );
        assert_eq!(accept_permission.r#type.clone(), PermissionType::Admin);
        assert_eq!(accept_permission.user_id.unwrap(), new_user2.id.clone());
        assert_eq!(
            accept_permission.user_email.unwrap(),
            new_user2.email.clone()
        );
        assert!(accept_permission.accepted);

        let workspace_owner = pool
            .get_workspace_owner(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace_owner.id, new_user.id);

        //get permission by use id
        let permission_by_user1_id = pool
            .get_permission(new_user.id.clone(), new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        let permission_by_user2_id = pool
            .get_permission(new_user2.id.clone(), new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        let permission_by_user3_id = pool
            .get_permission(new_user3.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        assert_eq!(permission_by_user1_id, PermissionType::Owner);
        assert_eq!(permission_by_user2_id, PermissionType::Admin);
        assert_eq!(permission_by_user3_id, None);

        //get user workspace by user id
        let user1_workspace = pool.get_user_workspaces(new_user.id.clone()).await.unwrap();
        let user2_workspace = pool
            .get_user_workspaces(new_user2.id.clone())
            .await
            .unwrap();
        assert_eq!(user1_workspace.len(), 1);
        assert_eq!(user2_workspace.len(), 1);
        assert_eq!(
            user1_workspace.get(0).unwrap().id,
            user2_workspace.get(0).unwrap().id
        );

        //get workspace members
        let workspace_members = pool
            .get_workspace_members(new_workspace.id.clone())
            .await
            .unwrap();
        assert_eq!(workspace_members.len(), 2);

        let member1 = workspace_members.get(0).unwrap().user.clone();
        let member2 = workspace_members.get(1).unwrap().user.clone();
        if let UserCred::Registered(user) = member1 {
            assert_eq!(user.id, new_user.id);
        } else {
            panic!("get workspace members failed, owner is not registered");
        }
        if let UserCred::Registered(user2) = member2 {
            assert_eq!(user2.id, new_user2.id);
        } else {
            panic!("get workspace members failed, member is not registered");
        }

        //get user in workspace by email

        let user1_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user.email.clone())
            .await
            .unwrap();
        let user2_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user2.email.clone())
            .await
            .unwrap();
        let user3_not_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user3.email.clone())
            .await
            .unwrap();
        assert!(user1_in_workspace.in_workspace);
        assert!(user2_in_workspace.in_workspace);
        assert!(!user3_not_in_workspace.in_workspace);

        //can read workspace
        let owner_can_read_workspace = pool
            .can_read_workspace(new_user.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        let member_can_read_workspace = pool
            .can_read_workspace(new_user2.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        let another_can_not_read_workspace = pool
            .can_read_workspace(new_user3.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        assert_eq!(owner_can_read_workspace, true);
        assert_eq!(member_can_read_workspace, true);
        assert_eq!(another_can_not_read_workspace, false);

        //delete permission
        let is_deleted = pool
            .delete_permission(new_permission.0.clone())
            .await
            .unwrap();
        assert!(is_deleted);
        let workspace_owner = pool
            .get_workspace_owner(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace_owner.id, new_user.id);
        //delete permission by query
        let _new_permission = pool
            .create_permission(
                &new_user2.email.clone(),
                new_workspace.id.clone(),
                PermissionType::Admin,
            )
            .await
            .unwrap()
            .unwrap();

        let is_deleted_by_query = pool
            .delete_permission_by_query(new_user2.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        assert!(is_deleted_by_query);
        let workspace_owner = pool
            .get_workspace_owner(new_workspace.id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workspace_owner.id, new_user.id);
        //if in workspace after delete permission

        let user1_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user.email.clone())
            .await
            .unwrap();
        let user2_not_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user2.email.clone())
            .await
            .unwrap();
        let user3_not_in_workspace = pool
            .get_user_in_workspace_by_email(new_workspace.id.clone(), &new_user3.email.clone())
            .await
            .unwrap();
        assert!(user1_in_workspace.in_workspace);
        assert!(!user2_not_in_workspace.in_workspace);
        assert!(!user3_not_in_workspace.in_workspace);

        //can read workspace after delete permission
        let user1_can_read_workspace = pool
            .can_read_workspace(new_user.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        let user2_can_read_workspace = pool
            .can_read_workspace(new_user2.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        let user3_can_not_read_workspace = pool
            .can_read_workspace(new_user3.id.clone(), new_workspace.id.clone())
            .await
            .unwrap();
        assert_eq!(user1_can_read_workspace, true);
        assert_eq!(user2_can_read_workspace, false);
        assert_eq!(user3_can_not_read_workspace, false);
        Ok(())
    }
}
