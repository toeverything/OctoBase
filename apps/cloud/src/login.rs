// use async_trait::async_trait;
// use cloud_database::{GoogleClaims, UserWithNonce};
// use sqlx::{query, query_as};

// use crate::context::Context;

// #[async_trait]
// pub trait ThirdPartyLogin {
//     async fn google_user_login(&self, claims: &GoogleClaims) -> Result<UserWithNonce, ()>;
// }

// #[async_trait]
// impl ThirdPartyLogin for Context {
//     async fn google_user_login(&self, claims: &GoogleClaims) -> Result<UserWithNonce, ()> {
//         self.db.google_user_login(&claims).await
//     }
// }
