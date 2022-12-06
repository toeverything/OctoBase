use sqlx::{query, query_as, FromRow, Postgres, Transaction};

use crate::{
    context::Context,
    model::{
        CreatePermission, CreateWorkspace, Exist, Id, Permission, PermissionType, UpdateWorkspace,
        UserCredType, Workspace, WorkspaceDetail, WorkspaceType, WorkspaceWithPermission,
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
            user_cred TEXT NOT NULL,
            user_cred_type SMAILLINT NOT NULL,
            type SMALLINT NOT NULL,
            accepted BOOL DEFAULT True,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (workspace_id, user_cred)
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table permissions failed");

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

        let stmt = "SELECT type FROM permissions WHERE user_cred = $1 and workspace_id = $2";

        query_as::<_, Permission>(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .fetch_optional(&self.db)
            .await
            .map(|p| p.map(|p| p.type_))
    }

    pub async fn get_workspace_by_id(&self, workspace_id: i32) -> sqlx::Result<WorkspaceDetail> {
        let get_workspace = "SELECT id,public,type,created_at FROM workspaces WHERE id = $1;";

        let workspace = query_as::<_, Workspace>(&get_workspace)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await?;

        #[derive(FromRow)]
        struct UserId {
            user_cred: String,
        }

        let get_owner = format!(
            "SELECT user_cred FROM permissions WHERE workspace_id = $1 AND type = {}",
            PermissionType::Owner as i16
        );

        let owner = query_as::<_, UserId>(&get_owner)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await?
            .user_cred;

        #[derive(FromRow)]
        struct MemberCount {
            count: i32,
        }

        let get_member_count = "SELECT COUNT(permission.user_cred)
            FROM permissions
            WHERE workspace_id = $1 and accepted = True";

        let member_count = query_as::<_, MemberCount>(get_member_count)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await?
            .count;

        Ok(WorkspaceDetail {
            owner,
            member_count,
            workspace,
        })
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
            .fetch_one(&mut *trx)
            .await?;

        let create_permission = format!(
            "INSERT INTO permissions
                (user_cred, user_cred_type, workspace_id, type, accepted)
                VALUES ($1, {}, $2, {}, True);",
            UserCredType::Id as i16,
            PermissionType::Owner as i16
        );

        query(&create_permission)
            .bind(user_id)
            .bind(workspace.id)
            .execute(&mut *trx)
            .await?;

        Ok(workspace)
    }

    pub async fn init_user(
        &self,
        user_id: &str,
    ) -> sqlx::Result<Result<Workspace, CreatePrivateWorkspaceError>> {
        let mut trx = self.db.begin().await?;
        let get_private_workspace = format!(
            "SELECT EXISTS (
                SELECT 1 FROM permissions
                INNER JOIN workspaces
                  ON permissions.workspace_id = workspaces.id
                WHERE permissions.user_cred = $1 and permissions.type = {} and workspaces.type = {}
            )",
            PermissionType::Owner as i16,
            WorkspaceType::Private as i16
        );

        let exist = query_as::<_, Exist>(&get_private_workspace)
            .bind(user_id)
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
            WHERE id = $2 and type = {}
            RETURNING id,public,type,created_at;",
            WorkspaceType::Normal as i16
        );

        query_as::<_, Workspace>(&update_workspace)
            .bind(workspace_id)
            .bind(data.public)
            .fetch_one(&self.db)
            .await
    }

    pub async fn get_user_workspaces(
        &self,
        user_id: &str,
    ) -> sqlx::Result<Vec<WorkspaceWithPermission>> {
        let stmt = "SELECT 
            workspaces.id,workspaces.public,workspaces.created_at,workspaces.type,
            permissions.type as permission
        FROM permissions
        INNER JOIN workspaces
          ON permissions.workspace_id = workspaces.id
        WHERE user_cred = $1";

        query_as::<_, WorkspaceWithPermission>(&stmt)
            .bind(user_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn get_workspace_members(&self, workspace_id: i32) -> sqlx::Result<Vec<Permission>> {
        let stmt = "SELECT 
            id, workspace_id, type, user_cred, user_cred_type, accepted, created_at
            FROM permissions
            WHERE workspace_id = $1";

        query_as::<_, Permission>(stmt)
            .bind(workspace_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn create_permission(
        &self,
        data: &CreatePermission,
        workspace_id: i32,
        permission_type: PermissionType,
    ) -> sqlx::Result<Option<i32>> {
        let stmt = "INSERT INTO permissions (user_cred, user_cred_type, workspace_id, type) 
                VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            RETURNING id";

        query_as::<_, Id>(&stmt)
            .bind(&data.user_cred)
            .bind(data.user_cred_type as i32)
            .bind(workspace_id)
            .bind(permission_type as i16)
            .fetch_optional(&self.db)
            .await
            .map(|i| i.map(|i| i.id))
    }

    pub async fn accept_permission(
        &self,
        user_id: &str,
        permission_id: i32,
    ) -> sqlx::Result<Option<Permission>> {
        let stmt = format!(
            "UPDATE permissions
                SET 
            WHERE id = $1 and user_cred = $2 and user_cred_type = {}
            RETURNING id, user_cred, workspace_id, type, accepted, created_at",
            UserCredType::Id as i32
        );

        query_as::<_, Permission>(&stmt)
            .bind(permission_id)
            .bind(user_id)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn delete_permission(&self, permission_id: i32) -> sqlx::Result<()> {
        let stmt = "DELETE FROM permissions where permission_id = $1";

        query(&stmt).bind(permission_id).execute(&self.db).await?;

        Ok(())
    }
}
