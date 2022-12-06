use aes_gcm::{Aes256Gcm, KeyInit};
use lettre::message::Mailbox;
use lettre::AsyncSmtpTransport;
use lettre::{transport::smtp::authentication::Credentials, Tokio1Executor};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::model::User;

pub struct MailContext {
    pub client: AsyncSmtpTransport<Tokio1Executor>,
    pub mail_box: Mailbox,
    pub title: String,
}

pub struct Context {
    pub aes_key: Aes256Gcm,
    pub http_client: Client,
    pub mail: MailContext,
    pub db: PgPool,
}

impl Context {
    pub async fn new() -> Context {
        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");

        let db = PgPool::connect(&db_env).await.expect("wrong database URL");

        let aes_env = dotenvy::var("AES_KEY").expect("should provide AES key");

        let mut hasher = Sha256::new();
        hasher.update(aes_env.as_bytes());
        let hash = hasher.finalize();

        let aes_key = Aes256Gcm::new_from_slice(&hash[..]).unwrap();

        let mail_name = dotenvy::var("MAIL_ACCOUNT").expect("should provide email name");
        let mail_password = dotenvy::var("MAIL_PASSWORD").expect("should provide email password");

        let creds = Credentials::new(mail_name, mail_password);

        let mail_provider = dotenvy::var("MAIL_PROVIDER").expect("should provide email provider");

        // Open a remote connection to gmail
        let mail = {
            let client = AsyncSmtpTransport::<Tokio1Executor>::relay(&mail_provider)
                .unwrap()
                .credentials(creds)
                .build();

            let mail_from = dotenvy::var("MAIL_FROM").expect("should provide email from");
            let mail_box = mail_from.parse().expect("shoud provide valid mail from");
            let title = dotenvy::var("MAIL_TITLE").expect("should provide email title");
            MailContext {
                client,
                mail_box,
                title,
            }
        };

        let ctx = Self {
            db,
            aes_key,
            mail,
            http_client: Client::new(),
        };

        ctx.init_db().await;

        ctx
    }

    pub async fn get_users_by_id(&self, id: Vec<String>) -> Vec<User> {
        if id.is_empty() {
            return Vec::new();
        }

        Vec::new()
    }

    pub async fn get_user_by_id(&self, email: String) -> Option<User> {
        None
    }

    pub async fn get_user_by_email(&self, email: String) -> Option<User> {
        None
    }
}
