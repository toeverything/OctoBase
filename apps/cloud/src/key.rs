use crate::utils::CacheControl;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use axum::http::header::CACHE_CONTROL;
use chrono::{NaiveDateTime, Utc};
use cloud_database::{Claims, GoogleClaims};
use jsonwebtoken::{decode, decode_header, encode, DecodingKey, EncodingKey, Header, Validation};
use jwst_logger::info;
use rand::{thread_rng, Rng};
use reqwest::{Client, RequestBuilder};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use x509_parser::prelude::parse_x509_pem;

pub struct KeyContext {
    jwt_encode: EncodingKey,
    pub jwt_decode: DecodingKey,
    aes: Aes256Gcm,
}

impl KeyContext {
    pub fn new(key: String) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();

        let aes = Aes256Gcm::new_from_slice(&hash[..]).unwrap();

        let jwt_encode = EncodingKey::from_secret(key.as_bytes());
        let jwt_decode = DecodingKey::from_secret(key.as_bytes());
        Self {
            jwt_encode,
            jwt_decode,
            aes,
        }
    }

    pub fn sign_jwt(&self, user: &Claims) -> String {
        encode(&Header::default(), user, &self.jwt_encode).expect("encode JWT error")
    }

    #[allow(unused)]
    pub fn decode_jwt(&self, token: &str) -> Option<Claims> {
        if let Ok(res) = decode::<Claims>(token, &self.jwt_decode, &Validation::default()) {
            Some(res.claims)
        } else {
            None
        }
    }

    pub fn encrypt_aes(&self, input: &[u8]) -> Vec<u8> {
        let rand_data: [u8; 12] = thread_rng().gen();
        let nonce = Nonce::from_slice(&rand_data);

        let mut encrypted = self.aes.encrypt(nonce, input).unwrap();
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

        Ok(self.aes.decrypt(nonce, content).ok())
    }
}

pub struct Endpoint {
    endpoint: Option<String>,
    password: Option<String>,
    http_client: Client,
}

impl Endpoint {
    fn new() -> Self {
        Self {
            endpoint: dotenvy::var("GOOGLE_ENDPOINT").ok(),
            password: dotenvy::var("GOOGLE_ENDPOINT_PASSWORD").ok(),
            http_client: Client::new(),
        }
    }

    #[inline]
    fn endpoint(&self) -> String {
        format!(
            "{}/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com",
            self.endpoint.as_deref().unwrap_or("www.googleapis.com")
        )
    }

    fn connect(&self) -> RequestBuilder {
        if let Some(password) = &self.password {
            self.http_client
                .get(self.endpoint())
                .basic_auth("affine", Some(password))
        } else {
            self.http_client.get(self.endpoint())
        }
    }
}

pub struct FirebaseContext {
    endpoint: Endpoint,
    project_ids: Vec<String>,
    expires: NaiveDateTime,
    pub_key: HashMap<String, DecodingKey>,
}

impl FirebaseContext {
    pub fn new(project_ids: Vec<String>) -> Self {
        Self {
            endpoint: Endpoint::new(),
            project_ids,
            expires: NaiveDateTime::MIN,
            pub_key: HashMap::new(),
        }
    }

    async fn init_from_firebase(&mut self) {
        let resp = self.endpoint.connect().send().await.unwrap();

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

        self.expires = expires;
        self.pub_key = pub_key;
    }

    pub async fn decode_google_token(&mut self, token: String) -> Option<GoogleClaims> {
        let header = decode_header(&token).ok()?;

        if self.expires < Utc::now().naive_utc() {
            self.init_from_firebase().await;
        }
        let key = self.pub_key.get(&header.kid?)?;

        let mut validation = Validation::new(header.alg);

        validation.set_audience(&self.project_ids);

        match decode::<GoogleClaims>(&token, key, &validation).map(|d| d.claims) {
            Ok(c) => Some(c),
            Err(e) => {
                info!("invalid token {}", e);
                None
            }
        }
    }
}
