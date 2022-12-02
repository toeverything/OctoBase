use std::ops::DerefMut;

use sqlx::{query, query_as, FromRow, Postgres, Transaction};

use crate::{
    context::Context,
    model::{
        CreatePermission, CreateWorkspace, Exist, PermissionType, ShareUrl, UpdateWorkspace,
        Workspace, WorkspaceType, WorkspaceWithPermission,
    },
};

pub enum CreatePrivateWorkspaceError {
    Exist,
}

impl Context {
    pub async fn init_db(&self) {
        let stmt = "CREATE TABLE IF NOT EXISTS workspaces (
            id SERIAL PRIMARY KEY,
            public BOOL NOT NULL,
            type SMALLINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table workspaces failed");

        let stmt = "CREATE TABLE IF NOT EXISTS permissions (
            id SERIAL PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id),
            user_id TEXT NOT NULL,
            type SMALLINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table permissions failed");

        let stmt = "CREATE TABLE IF NOT EXISTS share_urls (
                id SERIAL PRIMARY KEY,
                workspace_id INTEGER REFERENCES workspaces(id),
                url TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table share_urls failed");

        let stmt = "CREATE TABLE IF NOT EXISTS workspace_data (
                id SERIAL PRIMARY KEY,
                workspace_id INTEGER REFERENCES workspaces(id),
                workspace_data bytea,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table workspace_url failed");
    }

    pub async fn can_read_workspace(&self, user_id: &str, workspace_id: i32) -> sqlx::Result<bool> {
        #[derive(FromRow)]
        struct Public {
            public: bool,
        }

        let public = "SELECT public FROM workspace WHERE id = $1";

        let public = query_as::<_, Public>(&public)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await?
            .public;

        if public {
            return Ok(true);
        }

        self.get_permission(user_id, workspace_id)
            .await
            .map(|p| p.is_some())
    }

    pub async fn get_permission(
        &self,
        user_id: &str,
        workspace_id: i32,
    ) -> sqlx::Result<Option<PermissionType>> {
        #[derive(FromRow)]
        struct Permission {
            #[sqlx(rename = "type")]
            type_: PermissionType,
        }

        let stmt = "SELECT type FROM permissions WHERE user_id = $1 and workspace_id = $2";

        let permission = query_as::<_, Permission>(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await;

        match permission {
            Ok(p) => Ok(Some(p.type_)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn get_workspace_by_id(&self, workspace_id: i32) -> sqlx::Result<Workspace> {
        let stmt = "SELECT id,public,type,created_at FROM workspaces WHERE id = $1;";

        query_as::<_, Workspace>(&stmt)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await
    }

    async fn create_workspace(
        trx: &mut Transaction<'static, Postgres>,
        user_id: &str,
        ws_type: WorkspaceType,
    ) -> sqlx::Result<Workspace> {
        let create_workspace = format!(
            "INSERT INTO workspaces 
                (public, type) VALUES (false, $1) 
            RETURNING id,public,created_at,type;",
        );

        let workspace = query_as::<_, Workspace>(&create_workspace)
            .bind(ws_type as i16)
            .fetch_one(trx.deref_mut())
            .await?;

        let create_permission = format!(
            "INSERT INTO permissions (user_id, workspace_id, type) VALUES ($1, $2, {});",
            PermissionType::Owner as i32
        );

        query(&create_permission)
            .bind(user_id)
            .bind(workspace.id)
            .execute(trx.deref_mut())
            .await?;

        Ok(workspace)
    }

    pub async fn create_private_workspace(
        &self,
        user_id: &str,
    ) -> sqlx::Result<Result<Workspace, CreatePrivateWorkspaceError>> {
        let mut trx = self.db.begin().await?;
        let get_private_workspace = "SELECT EXISTS (
                SELECT 1 FROM permissions
                INNER JOIN workspaces
                  ON permissions.workspace_id = workspaces.id
                WHERE permissions.user_id = $1 and permissions.type = $2 and workspaces.type = $3
            )";

        let exist = query_as::<_, Exist>(&get_private_workspace)
            .bind(user_id)
            .bind(PermissionType::Owner)
            .bind(WorkspaceType::Private)
            .fetch_one(&mut trx)
            .await?
            .exists;

        if exist {
            return Ok(Err(CreatePrivateWorkspaceError::Exist));
        }

        let workspace = Self::create_workspace(&mut trx, user_id, WorkspaceType::Private).await?;

        trx.commit().await?;

        Ok(Ok(workspace))
    }

    pub async fn create_normal_workspace(
        &self,
        user_id: &str,
        data: CreateWorkspace,
    ) -> sqlx::Result<Workspace> {
        let mut trx = self.db.begin().await?;
        let workspace = Self::create_workspace(&mut trx, user_id, WorkspaceType::Normal).await?;

        trx.commit().await?;

        Ok(workspace)
    }

    pub async fn update_workspace(
        &self,
        workspace_id: i32,
        data: UpdateWorkspace,
    ) -> sqlx::Result<Workspace> {
        let update_workspace = format!(
            "UPDATE workspaces 
                SET public = $1
            WHERE id = $2
            RETURNING id,public,type,created_at;",
        );

        query_as::<_, Workspace>(&update_workspace)
            .bind(workspace_id)
            .bind(data.public)
            .fetch_one(&self.db)
            .await
    }

    pub async fn get_workspace(&self, user_id: &str) -> sqlx::Result<Vec<WorkspaceWithPermission>> {
        let stmt = "SELECT 
            workspaces.id,workspaces.public,workspaces.created_at,workspaces.type,
            permissions.type as permission
        FROM permissions
        INNER JOIN workspaces
          ON permissions.workspace_id = workspaces.id
        WHERE user_id = $1";

        query_as::<_, WorkspaceWithPermission>(&stmt)
            .bind(user_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn create_permission(
        &self,
        user_id: &str,
        data: CreatePermission,
    ) -> sqlx::Result<()> {
        let stmt = "
            INSERT INTO permissions (user_id, workspace_id, type) 
                VALUES ($1, $2, $3)
                RETURNGI";

        query(&stmt)
            .bind(user_id)
            .bind(data.workspace_id)
            .bind(PermissionType::Write as i16)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    pub async fn delete_permission(&self, user_id: &str, workspace_id: i32) -> sqlx::Result<()> {
        let stmt = "DELETE FROM permissions where user_id = $1 and workspace_id = $2";

        query(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    pub async fn create_share_url(&self, workspace_id: i32, url: String) -> sqlx::Result<ShareUrl> {
        let stmt = "INSERT INTO 
            share_urls (workspace_id, url) VALUES ($1, $2)
            ON CONFLICT workspace_id DO NOTHING
            RETURNING id, workspace_id, url, create_at";

        query_as::<_, ShareUrl>(&stmt)
            .bind(workspace_id)
            .bind(url)
            .fetch_one(&self.db)
            .await
    }

    pub async fn delete_share_url(&self, workspace_id: i32) -> sqlx::Result<()> {
        let stmt = "DELETE FROM share_urls WHERE workspace_id = $1";

        query(&stmt).bind(workspace_id).execute(&self.db).await?;

        Ok(())
    }

    pub async fn get_workspace_by_share_url(&self, url: String) -> sqlx::Result<ShareUrl> {
        let stmt = "SELECT id, url, create_at WHERE workspace_id = $1";

        query_as::<_, ShareUrl>(&stmt)
            .bind(url)
            .fetch_one(&self.db)
            .await
    }
}
