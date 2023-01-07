use async_trait::async_trait;
#[cfg(feature = "postgres")]
use jwst_storage::PostgresDBContext;
#[cfg(feature = "sqlite")]
use jwst_storage::SqliteDBContext;
use jwst_storage::{GoogleClaims, UserWithNonce};
use sqlx::{query, query_as};

use crate::context::Context;

#[async_trait]
pub trait ThirdPartyLogin {
    async fn google_user_login(&self, claims: &GoogleClaims) -> sqlx::Result<UserWithNonce>;
}

#[async_trait]
impl ThirdPartyLogin for Context {
    async fn google_user_login(&self, claims: &GoogleClaims) -> sqlx::Result<UserWithNonce> {
        let mut trx = self.db.db.begin().await?;
        let update_user = "UPDATE users
    SET
        name = $1,
        email = $2,
        avatar_url = $3
    FROM google_users
    WHERE google_users.google_id = $4 AND google_users.user_id = users.id
    RETURNING users.id, users.name, users.email, users.avatar_url,
        users.token_nonce, users.created_at";

        if let Some(user) = query_as::<_, UserWithNonce>(update_user)
            .bind(&claims.name)
            .bind(&claims.email)
            .bind(&claims.picture)
            .bind(&claims.user_id)
            .fetch_optional(&mut trx)
            .await?
        {
            return Ok(user);
        }

        let create_user = "INSERT INTO users 
        (name, email, avatar_url) 
        VALUES ($1, $2, $3)
    ON CONFLICT (email) DO NOTHING
    RETURNING id, name, email, avatar_url, token_nonce, created_at";

        let user = query_as::<_, UserWithNonce>(create_user)
            .bind(&claims.name)
            .bind(&claims.email)
            .bind(&claims.picture)
            .fetch_one(&mut trx)
            .await?;

        let create_google_user = "INSERT INTO google_users 
        (user_id, google_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING";

        query(create_google_user)
            .bind(user.user.id)
            .bind(&claims.user_id)
            .execute(&mut trx)
            .await?;

        #[cfg(feature = "postgres")]
        PostgresDBContext::update_cred(&mut trx, user.user.id, &user.user.email).await?;
        #[cfg(feature = "sqlite")]
        SqliteDBContext::update_cred(&mut trx, user.user.id, &user.user.email).await?;

        trx.commit().await?;

        Ok(user)
    }
}
