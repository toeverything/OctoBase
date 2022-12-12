use std::collections::HashMap;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use chrono::{NaiveDateTime, Utc};
use http::header::CACHE_CONTROL;
use jsonwebtoken::{decode_header, DecodingKey, EncodingKey};
use lettre::message::Mailbox;
use lettre::AsyncSmtpTransport;
use lettre::{transport::smtp::authentication::Credentials, Tokio1Executor};
use rand::{thread_rng, Rng};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::info;
use x509_parser::prelude::parse_x509_pem;

use crate::model::{Claims, GoogleClaims};
use crate::utils::CacheControl;

pub struct KeyContext {
    pub jwt_encode: EncodingKey,
    pub jwt_decode: DecodingKey,
    pub aes: Aes256Gcm,
}

struct FirebaseContext {
    expires: NaiveDateTime,
    pub_key: HashMap<String, DecodingKey>,
}

pub struct MailContext {
    pub client: AsyncSmtpTransport<Tokio1Executor>,
    pub mail_box: Mailbox,
    pub title: String,
}

pub struct Context {
    pub key: KeyContext,
    pub http_client: Client,
    firebase: RwLock<FirebaseContext>,
    pub mail: MailContext,
    pub db: PgPool,
}

impl Context {
    pub async fn new() -> Context {
        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");

        let db = PgPool::connect(&db_env).await.expect("wrong database URL");

        let key = {
            let aes_env = dotenvy::var("AES_KEY").expect("should provide AES key");

            let mut hasher = Sha256::new();
            hasher.update(aes_env.as_bytes());
            let hash = hasher.finalize();

            let aes = Aes256Gcm::new_from_slice(&hash[..]).unwrap();

            let jwt_env = dotenvy::var("JWT_KEY").expect("should provide JWT key");

            let jwt_encode = EncodingKey::from_secret(jwt_env.as_bytes());
            let jwt_decode = DecodingKey::from_secret(jwt_env.as_bytes());
            KeyContext {
                jwt_encode,
                jwt_decode,
                aes,
            }
        };

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

        let firebase = RwLock::new(FirebaseContext {
            expires: NaiveDateTime::MIN,
            pub_key: HashMap::new(),
        });

        let ctx = Self {
            db,
            key,
            firebase,
            mail,
            http_client: Client::new(),
        };

        ctx.init_db().await;

        ctx
    }

    async fn init_from_firebase(&self) -> RwLockReadGuard<FirebaseContext> {
        let req = self.http_client.get(
            "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com",
        )
        .send()
        .await.unwrap();

        let now = Utc::now().naive_utc();
        let cache = req.headers().get(CACHE_CONTROL).unwrap().to_str().unwrap();
        let cache = CacheControl::parse(cache).unwrap();
        let expires = now + cache.max_age.unwrap();

        let body: HashMap<String, String> = req.json().await.unwrap();

        let pub_key = body
            .into_iter()
            .map(|(key, value)| {
                let (_, pem) = parse_x509_pem(value.as_bytes()).expect("decode PEM error");
                let cert = pem.parse_x509().expect("decode certificate error");

                let pub_key = pem::encode(&pem::Pem {
                    tag: String::from("PUBLIC KEY"),
                    contents: cert.public_key().raw.to_vec(),
                });
                let decode = DecodingKey::from_rsa_pem(pub_key.as_bytes()).unwrap();

                (key, decode)
            })
            .collect();

        let mut state = self.firebase.write().await;

        state.expires = expires;
        state.pub_key = pub_key;

        state.downgrade()
    }

    pub async fn decode_google_token(&self, token: String) -> Option<GoogleClaims> {
        use jsonwebtoken::{decode, Validation};
        let header = decode_header(&token).ok()?;
        let state = self.firebase.read().await;

        let state = if state.expires < Utc::now().naive_utc() {
            drop(state);
            self.init_from_firebase().await
        } else {
            state
        };
        let key = state.pub_key.get(&header.kid?)?;

        match decode::<GoogleClaims>(&token, key, &Validation::new(header.alg)).map(|d| d.claims) {
            Ok(c) => Some(c),
            Err(e) => {
                info!("invalid token {}", e);
                None
            }
        }
    }

    pub fn sign_jwt(&self, user: &Claims) -> String {
        use jsonwebtoken::{encode, Header};
        encode(&Header::default(), user, &self.key.jwt_encode).expect("encode JWT error")
    }

    pub fn decode_jwt(&self, token: &str) -> Option<Claims> {
        use jsonwebtoken::{decode, Validation};
        if let Ok(res) = decode::<Claims>(token, &self.key.jwt_decode, &Validation::default()) {
            Some(res.claims)
        } else {
            None
        }
    }

    pub fn encrypt_aes(&self, input: &[u8]) -> Vec<u8> {
        let rand_data: [u8; 12] = thread_rng().gen();
        let nonce = Nonce::from_slice(&rand_data);

        let mut encrypted = self.key.aes.encrypt(nonce, input).unwrap();
        encrypted.extend(nonce);

        encrypted
    }

    pub fn decrypt_aes(&self, input: Vec<u8>) -> Option<Vec<u8>> {
        let (content, nonce) = input.split_at(input.len() - 12);

        let nonce = nonce.try_into().ok()?;

        self.key.aes.decrypt(nonce, content).ok()
    }
}
