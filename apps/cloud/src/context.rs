use sqlx::PgPool;

pub struct Context {
    pub hash_key: Vec<u8>,
    pub db: PgPool,
}

impl Context {
    pub async fn new() -> Context {
        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");

        let db = PgPool::connect(&db_env).await.expect("wrong database URL");

        let hash_env = dotenvy::var("HASH_KEY").expect("should provide hash key");

        let ctx = Self {
            db,
            hash_key: hash_env.into_bytes(),
        };

        ctx.init_db().await;

        ctx
    }
}
