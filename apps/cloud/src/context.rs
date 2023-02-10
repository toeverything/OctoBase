use super::constants;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use axum::extract::ws::Message;
use chrono::{NaiveDateTime, Utc};
use cloud_database::PostgresDBContext;
use cloud_database::{Claims, GoogleClaims};
use dashmap::DashMap;
use handlebars::Handlebars;
use http::header::CACHE_CONTROL;
use jsonwebtoken::{decode_header, DecodingKey, EncodingKey};
use jwst::{DocStorage, SearchResults, Workspace};
use jwst_logger::{error, info};
use jwst_storage::{BlobAutoStorage, DocAutoStorage};
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    Tokio1Executor,
};
use moka::future::Cache;
use rand::{thread_rng, Rng};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::Sender;
use tokio::sync::{RwLock, RwLockReadGuard};
use x509_parser::prelude::parse_x509_pem;

use crate::api::UserChannel;
use crate::utils::CacheControl;

pub struct KeyContext {
    pub jwt_encode: EncodingKey,
    pub jwt_decode: DecodingKey,
    pub aes: Aes256Gcm,
}

struct FirebaseContext {
    id: String,
    expires: NaiveDateTime,
    pub_key: HashMap<String, DecodingKey>,
}

pub struct MailContext {
    pub client: AsyncSmtpTransport<Tokio1Executor>,
    pub mail_box: Mailbox,
    pub template: Handlebars<'static>,
}

pub struct DocStore {
    pub storage: DocAutoStorage,
}

impl DocStore {
    async fn new() -> Self {
        let database_url = dotenvy::var("DATABASE_URL").expect("should provide doc storage path");

        Self {
            storage: DocAutoStorage::init_pool(&format!("{database_url}_docs"))
                .await
                .expect("Failed to init doc storage"),
        }
    }

    pub async fn get_workspace(&self, workspace_id: String) -> Arc<RwLock<Workspace>> {
        self.storage
            .get(workspace_id.clone())
            .await
            .expect("Failed to get workspace")
    }

    pub async fn full_migrate(&self, workspace_id: String, update: Option<Vec<u8>>) -> bool {
        if let Ok(workspace) = self.storage.get(workspace_id.clone()).await {
            let update = if let Some(update) = update {
                if let Err(e) = self.storage.delete(workspace_id.clone()).await {
                    error!("full_migrate write error: {}", e.to_string());
                    return false;
                };
                update
            } else {
                workspace.read().await.sync_migration()
            };
            if let Err(e) = self.storage.full_migrate(&workspace_id, update).await {
                error!("db write error: {}", e.to_string());
                return false;
            }
            return true;
        }
        false
    }
}

pub struct Context {
    pub key: KeyContext,
    pub site_url: String,
    pub http_client: Client,
    firebase: RwLock<FirebaseContext>,
    pub mail: MailContext,
    pub db: PostgresDBContext,
    pub blob: BlobAutoStorage,
    pub doc: DocStore,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub user_channel: UserChannel,
}

impl Context {
    pub async fn new() -> Context {
        let key = {
            let key_env = dotenvy::var("SIGN_KEY").expect("should provide AES key");

            let mut hasher = Sha256::new();
            hasher.update(key_env.as_bytes());
            let hash = hasher.finalize();

            let aes = Aes256Gcm::new_from_slice(&hash[..]).unwrap();

            let jwt_encode = EncodingKey::from_secret(key_env.as_bytes());
            let jwt_decode = DecodingKey::from_secret(key_env.as_bytes());
            KeyContext {
                jwt_encode,
                jwt_decode,
                aes,
            }
        };

        let mail_name = dotenvy::var("MAIL_ACCOUNT").expect("should provide email name");
        let mail_password = dotenvy::var("MAIL_PASSWORD").expect("should provide email password");

        let creds = Credentials::new(mail_name, mail_password);

        // Open a remote connection to gmail
        let mail = {
            let client = AsyncSmtpTransport::<Tokio1Executor>::relay(constants::MAIL_PROVIDER)
                .unwrap()
                .credentials(creds)
                .build();

            let mail_box = constants::MAIL_FROM
                .parse()
                .expect("should provide valid mail from");

            let mut template = Handlebars::new();
            template
                .register_template_string("MAIL_INVITE_TITLE", constants::MAIL_INVITE_TITLE)
                .expect("should provide valid email title");

            let invite_file = constants::StaticFiles::get("invite.html").unwrap();
            let invite_file = String::from_utf8_lossy(&invite_file.data);
            template
                .register_template_string("MAIL_INVITE_CONTENT", &invite_file)
                .expect("should provide valid email file");

            MailContext {
                client,
                mail_box,
                template,
            }
        };

        let firebase_id = dotenvy::var("FIREBASE_PROJECT_ID").expect("should provide Firebase ID");

        let firebase = RwLock::new(FirebaseContext {
            id: firebase_id,
            expires: NaiveDateTime::MIN,
            pub_key: HashMap::new(),
        });

        let db_env = dotenvy::var("DATABASE_URL").expect("should provide database URL");

        let blob = BlobAutoStorage::init_pool(&format!("{db_env}_blobs"))
            .await
            .expect("Cannot create database");

        let site_url = dotenvy::var("SITE_URL").expect("should provide site url");

        Self {
            db: PostgresDBContext::new(db_env).await,
            key,
            firebase,
            mail,
            http_client: Client::new(),
            doc: DocStore::new().await,
            blob,
            site_url,
            channel: DashMap::new(),
            user_channel: UserChannel::new(),
        }
    }

    async fn init_from_firebase(&self) -> RwLockReadGuard<FirebaseContext> {
        let client = if let Ok(endpoint) = dotenvy::var("GOOGLE_ENDPOINT") {
            let endpoint =
                format!("{endpoint}/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com");
            self.http_client.get(endpoint).basic_auth(
                "affine",
                Some(
                    dotenvy::var("GOOGLE_ENDPOINT_PASSWORD")
                        .expect("should provide google endpoint password"),
                ),
            )
        } else {
            let endpoint = "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";
            self.http_client.get(endpoint)
        };

        let req = client.send().await.unwrap();

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

        let mut validation = Validation::new(header.alg);

        validation.set_audience(&[&state.id]);

        match decode::<GoogleClaims>(&token, key, &validation).map(|d| d.claims) {
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

    pub fn decrypt_aes(&self, input: Vec<u8>) -> Result<Option<Vec<u8>>, &'static str> {
        if input.len() < 12 {
            return Err("an unexpected value");
        }
        let (content, nonce) = input.split_at(input.len() - 12);

        let Some(nonce) = nonce.try_into().ok() else {
            return Err("an unexpected value");
        };

        Ok(self.key.aes.decrypt(nonce, content).ok())
    }

    pub async fn search_workspace(
        &self,
        workspace_id: String,
        query_string: &str,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        let workspace_id = workspace_id.to_string();
        let workspace_arc_rw = self.doc.get_workspace(workspace_id.clone()).await;

        let search_results = workspace_arc_rw.write().await.search(query_string)?;

        Ok(search_results)
    }
}
