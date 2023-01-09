use super::*;
use sqlx::{query, query_as, FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(FromRow)]
struct PermissionQuery {
    #[sqlx(rename = "type")]
    type_: PermissionType,
}

pub struct PostgreSQL {
    pub db: PgPool,
}

impl PostgreSQL {
    pub async fn new(database: String) -> PostgreSQL {
        let db = PgPool::connect(&database)
            .await
            .expect("wrong database URL");
        let db_context = Self { db };
        db_context.init_db().await;
        db_context
    }

    pub async fn init_db(&self) {
        let stmt = "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            avatar_url TEXT,
            token_nonce SMALLINT DEFAULT 0,
            password TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (email)
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table users failed");

        let stmt = "CREATE TABLE IF NOT EXISTS google_users (
            id SERIAL PRIMARY KEY,
            user_id INTEGER REFERENCES users(id),
            google_id TEXT NOT NULL,
            UNIQUE (google_id)
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table google_users failed");

        let stmt = "CREATE TABLE IF NOT EXISTS workspaces (
            id BIGSERIAL PRIMARY KEY,
            uuid CHAR(36),
            public BOOL NOT NULL,
            type SMALLINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(uuid)
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table workspaces failed");

        let stmt = "CREATE TABLE IF NOT EXISTS permissions (
            id BIGSERIAL PRIMARY KEY,
            workspace_id CHAR(36),
            user_id INTEGER,
            user_email TEXT,
            type SMALLINT NOT NULL,
            accepted BOOL DEFAULT False,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(uuid),
            FOREIGN KEY(user_id) REFERENCES users(id),
            UNIQUE (workspace_id, user_id),
            UNIQUE (workspace_id, user_email)
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table permissions failed");
    }

    pub async fn get_user_by_email(&self, email: &str) -> sqlx::Result<Option<User>> {
        let stmt = "SELECT id, name, email, avatar_url, created_at FROM users WHERE email = $1";

        query_as::<_, User>(stmt)
            .bind(email)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn get_workspace_owner(&self, workspace_id: String) -> sqlx::Result<User> {
        let stmt = format!(
            "SELECT
                users.id, users.name, users.email, users.avatar_url, users.created_at
            FROM permissions
            INNER JOIN users
                ON permissions.user_id = users.id
            WHERE workspace_id = $1 AND type = {}",
            PermissionType::Owner as i16
        );
        query_as::<_, User>(&stmt)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await
    }

    pub async fn user_login(&self, login: UserLogin) -> sqlx::Result<Option<UserWithNonce>> {
        let stmt = "SELECT 
            id, name, email, avatar_url, token_nonce, created_at
        FROM users
        WHERE email = $1 AND password = $2";

        query_as::<_, UserWithNonce>(stmt)
            .bind(login.email)
            .bind(login.password)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn refresh_token(&self, token: RefreshToken) -> sqlx::Result<Option<UserWithNonce>> {
        let stmt = "SELECT 
            id, name, email, avatar_url, token_nonce, created_at
        FROM users
        WHERE id = $1 AND token_nonce = $2";

        query_as::<_, UserWithNonce>(stmt)
            .bind(token.user_id)
            .bind(token.token_nonce)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn verify_refresh_token(&self, token: &RefreshToken) -> sqlx::Result<bool> {
        let stmt = "SELECT True
        FROM users
        WHERE id = $1 AND token_nonce = $2";

        query(stmt)
            .bind(token.user_id)
            .bind(token.token_nonce)
            .fetch_optional(&self.db)
            .await
            .map(|r| r.is_some())
    }

    pub async fn update_cred(
        trx: &mut Transaction<'static, Postgres>,
        user_id: i32,
        user_email: &str,
    ) -> sqlx::Result<()> {
        let update_cred = "UPDATE permissions
        SET user_id = $1,
            user_email = NULL
        WHERE user_email = $2";

        query(update_cred)
            .bind(user_id)
            .bind(user_email)
            .execute(&mut *trx)
            .await?;

        Ok(())
    }

    pub async fn create_user(&self, user: CreateUser) -> sqlx::Result<Option<User>> {
        let mut trx = self.db.begin().await?;
        let create_user = "INSERT INTO users 
            (name, password, email, avatar_url)
            VALUES ($1, $2, $3, $4)
        ON CONFLICT (email) DO NOTHING
        RETURNING id, name, email, avatar_url, created_at";

        let Some(user) = query_as::<_, User>(create_user)
            .bind(user.name)
            .bind(user.password)
            .bind(user.email)
            .bind(user.avatar_url)
            .fetch_optional(&mut trx)
            .await? else {
                return Ok(None)
        };

        Self::create_workspace(&mut trx, user.id, WorkspaceType::Private).await?;

        Self::update_cred(&mut trx, user.id, &user.email).await?;

        trx.commit().await?;

        Ok(Some(user))
    }

    pub async fn get_workspace_by_id(
        &self,
        workspace_id: String,
    ) -> sqlx::Result<Option<WorkspaceDetail>> {
        let get_workspace =
            "SELECT uuid AS id, public, type, created_at FROM workspaces WHERE uuid = $1;";

        let workspace = query_as::<_, Workspace>(&get_workspace)
            .bind(workspace_id.clone())
            .fetch_optional(&self.db)
            .await?;

        let workspace = match workspace {
            Some(workspace) if workspace.type_ == WorkspaceType::Private => {
                return Ok(Some(WorkspaceDetail {
                    owner: None,
                    member_count: 0,
                    workspace,
                }))
            }
            Some(ws) => ws,
            None => return Ok(None),
        };

        let owner = self.get_workspace_owner(workspace_id.clone()).await?;

        let get_member_count = "SELECT COUNT(permissions.id) AS count
            FROM permissions
            WHERE workspace_id = $1 AND accepted = True";

        let member_count = query_as::<_, Count>(get_member_count)
            .bind(workspace_id)
            .fetch_one(&self.db)
            .await?
            .count;

        Ok(Some(WorkspaceDetail {
            owner: Some(owner),
            member_count,
            workspace,
        }))
    }

    pub async fn create_workspace(
        trx: &mut Transaction<'static, Postgres>,
        user_id: i32,
        ws_type: WorkspaceType,
    ) -> sqlx::Result<Workspace> {
        let create_workspace = format!(
            "INSERT INTO workspaces (public, type, uuid) VALUES (false, $1, $2) 
            RETURNING uuid AS id, public, created_at, type;",
        );
        let uuid = Uuid::new_v4();
        let workspace_id = uuid.to_string().replace("-", "_");

        let workspace = query_as::<_, Workspace>(&create_workspace)
            .bind(ws_type as i16)
            .bind(workspace_id)
            .fetch_one(&mut *trx)
            .await?;

        let create_permission = format!(
            "INSERT INTO permissions
                (user_id, workspace_id, type, accepted)
            VALUES ($1, $2, {}, True);",
            PermissionType::Owner as i16
        );

        query(&create_permission)
            .bind(user_id)
            .bind(workspace.id.clone())
            .execute(&mut *trx)
            .await?;

        Ok(workspace)
    }

    pub async fn create_normal_workspace(&self, user_id: i32) -> sqlx::Result<Workspace> {
        let mut trx = self.db.begin().await?;
        let workspace = Self::create_workspace(&mut trx, user_id, WorkspaceType::Normal).await?;

        trx.commit().await?;

        Ok(workspace)
    }

    pub async fn update_workspace(
        &self,
        workspace_id: String,
        data: UpdateWorkspace,
    ) -> sqlx::Result<Option<Workspace>> {
        let update_workspace = format!(
            "UPDATE workspaces
                SET public = $1
            WHERE uuid = $2 AND type = {}
            RETURNING uuid as id, public, type, created_at;",
            WorkspaceType::Normal as i16
        );

        query_as::<_, Workspace>(&update_workspace)
            .bind(data.public)
            .bind(workspace_id)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn delete_workspace(&self, workspace_id: String) -> sqlx::Result<bool> {
        let delete_workspace = format!(
            "DELETE FROM workspaces CASCADE
            WHERE uuid = $1 AND type = {}",
            WorkspaceType::Normal as i16
        );

        query(&delete_workspace)
            .bind(workspace_id)
            .execute(&self.db)
            .await
            .map(|q| q.rows_affected() != 0)
    }

    pub async fn get_user_workspaces(
        &self,
        user_id: i32,
    ) -> sqlx::Result<Vec<WorkspaceWithPermission>> {
        let stmt = "SELECT 
            workspaces.uuid AS id, workspaces.public, workspaces.created_at, workspaces.type,
            permissions.type as permission
        FROM permissions
        INNER JOIN workspaces
          ON permissions.workspace_id = workspaces.uuid
        WHERE user_id = $1";

        query_as::<_, WorkspaceWithPermission>(&stmt)
            .bind(user_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn get_workspace_members(&self, workspace_id: String) -> sqlx::Result<Vec<Member>> {
        let stmt = "SELECT 
            permissions.id, permissions.type, permissions.user_email,
            permissions.accepted, permissions.created_at,
            users.id as user_id, users.name as user_name, users.email as user_table_email, users.avatar_url,
            users.created_at as user_created_at
        FROM permissions
        LEFT JOIN users
            ON users.id = permissions.user_id
        WHERE workspace_id = $1";

        query_as::<_, Member>(stmt)
            .bind(workspace_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn get_permission(
        &self,
        user_id: i32,
        workspace_id: String,
    ) -> sqlx::Result<Option<PermissionType>> {
        let stmt = "SELECT type FROM permissions WHERE user_id = $1 AND workspace_id = $2";

        query_as::<_, PermissionQuery>(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .fetch_optional(&self.db)
            .await
            .map(|p| p.map(|p| p.type_))
    }

    pub async fn get_permission_by_permission_id(
        &self,
        user_id: i32,
        permission_id: i64,
    ) -> sqlx::Result<Option<PermissionType>> {
        let stmt = "SELECT type FROM permissions
        WHERE
            user_id = $1
        AND
            workspace_id = (SELECT workspace_id FROM permissions WHERE permissions.id = $2)
        ";

        query_as::<_, PermissionQuery>(&stmt)
            .bind(user_id)
            .bind(permission_id)
            .fetch_optional(&self.db)
            .await
            .map(|p| p.map(|p| p.type_))
    }

    pub async fn can_read_workspace(
        &self,
        user_id: i32,
        workspace_id: String,
    ) -> sqlx::Result<bool> {
        let stmt = "SELECT FROM permissions 
            WHERE user_id = $1
                AND workspace_id = $2
                OR (SELECT True FROM workspaces WHERE uuid = $2 AND public = True)";

        query(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .fetch_optional(&self.db)
            .await
            .map(|p| p.is_some())
    }

    pub async fn create_permission(
        &self,
        email: &str,
        workspace_id: String,
        permission_type: PermissionType,
    ) -> sqlx::Result<Option<(i64, UserCred)>> {
        let user = self.get_user_by_email(email).await?;

        let stmt = format!(
            "INSERT INTO permissions (user_id, user_email, workspace_id, type)
            SELECT $1, $2, $3, $4
            FROM workspaces
                WHERE workspaces.type = {} AND workspaces.uuid = $3
            ON CONFLICT DO NOTHING
            RETURNING id",
            WorkspaceType::Normal as i16
        );

        let query = query_as::<_, BigId>(&stmt);

        let (query, user) = match user {
            Some(user) => (
                query.bind(user.id).bind::<Option<String>>(None),
                UserCred::Registered(user),
            ),
            None => (
                query.bind::<Option<i32>>(None).bind(email),
                UserCred::UnRegistered {
                    email: email.to_owned(),
                },
            ),
        };

        let id = query
            .bind(workspace_id)
            .bind(permission_type as i16)
            .fetch_optional(&self.db)
            .await?;

        Ok(if let Some(id) = id {
            Some((id.id, user))
        } else {
            None
        })
    }

    pub async fn accept_permission(&self, permission_id: i64) -> sqlx::Result<Option<Permission>> {
        let stmt = "UPDATE permissions
                SET accepted = True
            WHERE id = $1
            RETURNING id, user_id, user_email, workspace_id, type, accepted, created_at";

        query_as::<_, Permission>(&stmt)
            .bind(permission_id)
            .fetch_optional(&self.db)
            .await
    }

    pub async fn delete_permission(&self, permission_id: i64) -> sqlx::Result<bool> {
        let stmt = "DELETE FROM permissions WHERE id = $1";

        query(&stmt)
            .bind(permission_id)
            .execute(&self.db)
            .await
            .map(|q| q.rows_affected() != 0)
    }

    pub async fn delete_permission_by_query(
        &self,
        user_id: i32,
        workspace_id: String,
    ) -> sqlx::Result<bool> {
        let stmt = format!(
            "DELETE FROM permissions
            WHERE user_id = $1 AND workspace_id = $2 AND type != {}",
            PermissionType::Owner as i16
        );

        query(&stmt)
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.db)
            .await
            .map(|q| q.rows_affected() != 0)
    }

    pub async fn get_user_in_workspace_by_email(
        &self,
        workspace_id: String,
        email: &str,
    ) -> sqlx::Result<UserInWorkspace> {
        let stmt = "SELECT 
            id, name, email, avatar_url, token_nonce, created_at
        FROM users";

        let user = query_as::<_, User>(stmt)
            .bind(workspace_id.clone())
            .fetch_optional(&self.db)
            .await?;

        Ok(if let Some(user) = user {
            let stmt = "SELECT True FROM permissions WHERE workspace_id = $1 AND user_id = $2";

            let in_workspace = query(stmt)
                .bind(workspace_id)
                .bind(user.id)
                .fetch_optional(&self.db)
                .await?
                .is_some();

            UserInWorkspace {
                user: UserCred::Registered(user),
                in_workspace,
            }
        } else {
            let stmt = "SELECT True FROM permissions WHERE workspace_id = $1 AND user_email = $2";

            let in_workspace = query_as::<_, User>(stmt)
                .bind(workspace_id.clone())
                .bind(email)
                .fetch_optional(&self.db)
                .await?
                .is_some();

            UserInWorkspace {
                user: UserCred::UnRegistered {
                    email: email.to_string(),
                },
                in_workspace,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #[ignore = "need postgres instance"]
    #[tokio::test]
    async fn database_create_tables() -> anyhow::Result<()> {
        use super::*;
        // let check_table_exists =
        // "IF EXISTS(SELECT 1 FROM sys.Tables WHERE  Name = N'users' AND Type = N'U')
        //     RETURN True
        // ELSE
        //     RETURN False";
        let db_context =
            PostgreSQL::new("postgresql://jwst:jwst@localhost:5432/jwst".to_string()).await;
        // clean db
        let drop_statement = "
        DROP TABLE IF EXISTS \"google_users\";
        DROP TABLE IF EXISTS \"permissions\";
        DROP TABLE IF EXISTS \"workspaces\";
        DROP TABLE IF EXISTS \"users\";
        ";
        for line in drop_statement.split("\n") {
            query(&line)
                .execute(&db_context.db)
                .await
                .expect("Drop all table in test failed");
        }
        db_context.init_db().await;
        // start test
        let new_user = db_context
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_user.id, 1);
        let new_user2 = db_context
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx2@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_user2.id, 2);
        let mut new_workspace = db_context
            .create_normal_workspace(new_user.id)
            .await
            .unwrap();
        let new_workspace2 = db_context
            .create_normal_workspace(new_user2.id)
            .await
            .unwrap();

        assert_eq!(new_workspace.id.len(), 36);
        assert_eq!(new_workspace.public, false);
        assert_eq!(new_workspace2.id.len(), 36);

        let new_workspace1_clone = db_context
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
            db_context
                .get_user_workspaces(new_user.id)
                .await
                .unwrap()
                // when create user, will auto create a private workspace, our created will be second one
                .get(1)
                .unwrap()
                .workspace
                .id
        );

        new_workspace = db_context
            .update_workspace(new_workspace.id, UpdateWorkspace { public: true })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_workspace.public, true);
        Ok(())
    }
}
