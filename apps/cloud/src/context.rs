use sqlx::{query, query_as, PgPool};

use crate::model::{CreateWorkspace, Workspace};

pub struct Context {
    pub db: PgPool,
}

impl Context {
    pub async fn new() -> Context {
        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");

        let db = PgPool::connect(&db_env).await.expect("wrong database URL");

        let ctx = Self { db };

        ctx.init_db().await;

        ctx
    }

    async fn init_db(&self) {
        let stmt = "CREATE TABLE IF NOT EXISTS workspaces (
            id SERIAL PRIMARY KEY,
            owner TEXT NOT NULL,
            public BOOL NOT NULL,
            name TEXT NOT NULL,
            avatar_url TEXT,
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
    }

    pub async fn get_workspace(&self, user_id: &str) -> sqlx::Result<Vec<Workspace>> {
        let stmt = "SELECT id,owner,public,type,created_at FROM workspaces WHERE owner = $1;";

        query_as::<_, Workspace>(&stmt)
            .bind(user_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn create_workspace(
        &self,
        user_id: &str,
        data: CreateWorkspace,
    ) -> sqlx::Result<Workspace> {
        let stmt = "INSERT INTO workspaces (owner, name, public, type) VALUES ($1, $2, $3, $4) 
        RETURNING id,owner,name,public,avatar_url,created_at,type;";

        query_as::<_, Workspace>(&stmt)
            .bind(user_id)
            .bind(data.name)
            .bind(data.public)
            .bind(data.type_)
            .fetch_one(&self.db)
            .await
    }

    pub async fn get_permited_workspace(&self, user_id: &str) -> sqlx::Result<Vec<Workspace>> {
        let stmt = "SELECT ";

        query_as::<_, Workspace>(&stmt)
            .bind(user_id)
            .fetch_all(&self.db)
            .await
    }

    pub async fn add_permission(&self, user_id: &str) {}

    pub async fn remove_permission(&self, user_id: &str) {}
}
