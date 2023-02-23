use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use axum::extract::ws::{self, Message};
use chrono::{NaiveDateTime, Utc};
use cloud_components::MailContext;
use cloud_database::CloudDatabase;
use cloud_database::{Claims, GoogleClaims};
use dashmap::DashMap;
use http::header::CACHE_CONTROL;
use jsonwebtoken::{decode_header, DecodingKey, EncodingKey};
use jwst::SearchResults;
use jwst_logger::{error, info};
use jwst_storage::JwstStorage;
use rand::{thread_rng, Rng};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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

pub struct Context {
    pub key: KeyContext,
    pub site_url: String,
    pub http_client: Client,
    firebase: RwLock<FirebaseContext>,
    pub mail: MailContext,
    pub db: CloudDatabase,
    pub storage: JwstStorage,
    pub channel: DashMap<(String, String, String), Sender<Message>>,
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

        let mail = MailContext::new(
            dotenvy::var("MAIL_ACCOUNT").expect("should provide email name"),
            dotenvy::var("MAIL_PASSWORD").expect("should provide email password"),
        );

        let firebase = RwLock::new(FirebaseContext {
            id: dotenvy::var("FIREBASE_PROJECT_ID").expect("should provide Firebase ID"),
            expires: NaiveDateTime::MIN,
            pub_key: HashMap::new(),
        });

        let db_env = dotenvy::var("DATABASE_URL").expect("should provide database URL");

        let site_url = dotenvy::var("SITE_URL").expect("should provide site url");

        let cloud_db = CloudDatabase::init_pool(&db_env)
            .await
            .expect("Cannot create cloud database");
        let storage = JwstStorage::new(&format!(
            "{}_binary",
            dotenvy::var("DATABASE_URL").expect("should provide doc storage path")
        ))
        .await
        .expect("Cannot create storage");

        Self {
            db: cloud_db,
            key,
            firebase,
            mail,
            http_client: Client::new(),
            storage,
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

        let resp = client.send().await.unwrap();

        let now = Utc::now().naive_utc();
        let cache = resp.headers().get(CACHE_CONTROL).unwrap().to_str().unwrap();
        let cache = CacheControl::parse(cache).unwrap();
        let expires = now + cache.max_age.unwrap();

        let body: HashMap<String, String> = resp.json().await.unwrap();

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

        match self.storage.get_workspace(workspace_id.clone()).await {
            Ok(workspace) => {
                let search_results = workspace.write().await.search(query_string)?;
                Ok(search_results)
            }
            Err(e) => {
                error!("cannot get workspace: {}", e);
                Err(Box::new(e))
            }
        }
    }

    // TODO: this should be moved to another module
    pub async fn close_websocket(&self, workspace: String, user: String) {
        let mut closed = vec![];
        for item in self.channel.iter() {
            let ((ws_id, user_id, id), tx) = item.pair();
            if &workspace == ws_id && user_id == &user {
                closed.push((ws_id.clone(), user_id.clone(), id.clone()));
                let _ = tx.send(ws::Message::Close(None)).await;
            }
        }
        for close in closed {
            let (ws_id, user_id, id) = close;
            self.channel.remove(&(ws_id, user_id, id));
        }
    }

    // TODO: this should be moved to another module
    pub async fn close_websocket_by_workspace(&self, workspace: String) {
        let mut closed = vec![];
        for item in self.channel.iter() {
            let ((ws_id, user_id, id), tx) = item.pair();
            if &workspace == ws_id {
                closed.push((ws_id.clone(), user_id.clone(), id.clone()));
                let _ = tx.send(ws::Message::Close(None)).await;
            }
        }
        for close in closed {
            let (ws_id, user_id, id) = close;
            self.channel.remove(&(ws_id, user_id, id));
        }
    }
}
