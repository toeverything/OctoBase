use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use axum::extract::ws::Message;
use chrono::{NaiveDateTime, Utc};
use dashmap::DashMap;
use handlebars::Handlebars;
use http::header::CACHE_CONTROL;
use jsonwebtoken::{decode_header, DecodingKey, EncodingKey};
use jwst::Workspace;
use jwst::{DocStorage, SearchResults, Workspace as JWSTWorkspace};
use jwst_logger::info;
use jwst_storage::{BlobFsStorage, Claims, DBContext, DocFsStorage, GoogleClaims};
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    Tokio1Executor,
};
use moka::future::Cache;
use rand::{thread_rng, Rng};
use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc::Sender, Mutex};
use tokio::sync::{RwLock, RwLockReadGuard};
use x509_parser::prelude::parse_x509_pem;

use crate::storage::DocDatabase;
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
    cache: Cache<i64, Arc<RwLock<JWSTWorkspace>>>,
    pub storage: DocFsStorage,
}

impl DocStore {
    async fn new() -> DocStore {
        let doc_env = dotenvy::var("DOC_STORAGE_PATH").expect("should provide doc storage path");

        DocStore {
            cache: Cache::new(1000),
            storage: DocFsStorage::new(Some(16), 500, Path::new(&doc_env).into()).await,
        }
    }

    pub async fn get_workspace(&self, id: i64) -> Option<Arc<RwLock<JWSTWorkspace>>> {
        self.cache
            .try_get_with(id, async move {
                self.storage
                    .get(id)
                    .await
                    .map(|f| Arc::new(RwLock::new(JWSTWorkspace::from_doc(f, id.to_string()))))
            })
            .await
            .ok()
    }

    pub fn try_get_workspace(&self, id: i64) -> Option<Arc<RwLock<JWSTWorkspace>>> {
        self.cache.get(&id)
    }
}

pub struct Context {
    pub key: KeyContext,
    pub site_url: String,
    pub http_client: Client,
    firebase: RwLock<FirebaseContext>,
    pub mail: MailContext,
    pub db: DBContext,
    pub blob: BlobFsStorage,
    pub doc: DocStore,
    pub workspace: DashMap<String, Mutex<Workspace>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub docs: DocDatabase,
}

pub enum ContextRequestError {
    WorkspaceNotFound {
        workspace_id: i64,
    },
    /// "Bad Request"
    BadUserInput {
        /// Something potentially helpful to the caller about what was wrong
        user_message: String,
    },
    /// "Internal Server Error" type of thing.
    /// It should probably not be surfaced to the user.
    Other(Box<dyn std::error::Error>),
}

impl ContextRequestError {
    fn other<E: std::error::Error + 'static>(value: E) -> Self {
        Self::Other(Box::new(value))
    }
}

impl From<Box<dyn std::error::Error>> for ContextRequestError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        Self::Other(value)
    }
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

        let mail_provider = dotenvy::var("MAIL_PROVIDER").expect("should provide email provider");

        // Open a remote connection to gmail
        let mail = {
            let client = AsyncSmtpTransport::<Tokio1Executor>::relay(&mail_provider)
                .unwrap()
                .credentials(creds)
                .build();

            let mail_from = dotenvy::var("MAIL_FROM").expect("should provide email from");
            let mail_box = mail_from.parse().expect("shoud provide valid mail from");
            let mut template = Handlebars::new();
            let invite_title =
                dotenvy::var("MAIL_INVITE_TITLE").expect("should provide email title");
            template
                .register_template_string("MAIL_INVITE_TITLE", invite_title)
                .expect("should privide valid email title");

            let invite_file =
                dotenvy::var("MAIL_INVITE_FILE").expect("should provide email content");

            template
                .register_template_file("MAIL_INVITE_CONTENT", &invite_file)
                .expect("should privide valid email file");

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

        let blob_env = dotenvy::var("BLOB_STORAGE_PATH").expect("should provide blob storage path");

        let blob = BlobFsStorage::new(Some(16), Path::new(&blob_env).into()).await;

        let site_url = dotenvy::var("SITE_URL").expect("should provide site url");

        let db_env = dotenvy::var("DATABASE_URL").expect("should provide databse URL");
        let ctx = Self {
            db: DBContext::new(db_env).await,
            key,
            firebase,
            mail,
            http_client: Client::new(),
            doc: DocStore::new().await,
            blob,
            site_url,
            workspace: DashMap::new(),
            channel: DashMap::new(),
            docs: DocDatabase::init_pool("jwst")
                .await
                .expect("Cannot create database"),
        };

        ctx
    }

    async fn init_from_firebase(&self) -> RwLockReadGuard<FirebaseContext> {
        let client = if let Ok(endpoint) = dotenvy::var("GOOGLE_ENDPOINT") {
            let endpoint = format!(
                "{}/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com",
                endpoint
            );
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

    pub fn decrypt_aes(&self, input: Vec<u8>) -> Option<Vec<u8>> {
        let (content, nonce) = input.split_at(input.len() - 12);

        let nonce = nonce.try_into().ok()?;

        self.key.aes.decrypt(nonce, content).ok()
    }

    pub async fn search_workspace(
        &self,
        id: i64,
        query_string: &str,
    ) -> Result<SearchResults, ContextRequestError> {
        let workspace_id = id.to_string();
        let workspace_arc_rw = self
            .docs
            .create_doc(&workspace_id)
            .await
            .map(|f| Arc::new(RwLock::new(JWSTWorkspace::from_doc(f, id.to_string()))))
            .ok()
            .ok_or_else(|| ContextRequestError::WorkspaceNotFound { workspace_id: id })?;

        let search_results = workspace_arc_rw.write().await.search(&query_string)?;

        Ok(search_results)
    }
}
