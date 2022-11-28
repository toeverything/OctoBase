use jsonwebtoken::DecodingKey;
use sqlx::{query, PgPool};

pub struct Context {
    pub pub_key: DecodingKey,
    pub db: PgPool,
}

impl Context {
    pub async fn new() -> Context {
        let pub_key = dotenvy::var("PUBLIC_KEY").expect("should provide public key");
        let pub_key =
            DecodingKey::from_rsa_pem(pub_key.as_bytes()).expect("decode public key error");

        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");

        let db = PgPool::connect(&db_env).await.expect("wrong database URL");

        let ctx = Self { pub_key, db };

        ctx.init_db().await;

        ctx
    }

    async fn init_db(&self) {
        let stmt = "CREATE TABLE IF NOT EXISTS workspaces (
            id SERIAL PRIMARY KEY,
            owner TEXT NOT NULL,
            public BOOL NOT NULL,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table workspaces failed");

        let stmt = "CREATE TABLE IF NOT EXISTS permissions (
            id SERIAL PRIMARY KEY,
            workspace INTEGER REFERENCES workspaces(id),
            user_id TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL
        );";
        query(&stmt)
            .execute(&self.db)
            .await
            .expect("create table permissions failed");
    }
}
