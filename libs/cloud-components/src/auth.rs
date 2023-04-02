use super::*;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use axum::headers::{CacheControl, HeaderMapExt};
use chrono::{Duration, NaiveDateTime, Utc};
use cloud_database::{Claims, FirebaseClaims};
use jsonwebtoken::{
    decode, decode_header, encode, errors::Error as JwtError, DecodingKey, EncodingKey, Header,
    Validation,
};
use jwst::{warn, Base64DecodeError, Base64Engine, URL_SAFE_ENGINE};
use pem::{encode as encode_pem, Pem};
use rand::{thread_rng, Rng};
use reqwest::{Client, RequestBuilder};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;
use x509_parser::prelude::{nom::Err as NomErr, parse_x509_pem, PEMError, X509Error};

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("base64 error")]
    Base64Error(#[from] Base64DecodeError),
    #[error("jwt error")]
    JwtError(#[from] JwtError),
    #[error("invalid data")]
    InvalidData,
}

pub type KeyResult<T> = Result<T, KeyError>;

pub struct KeyContext {
    jwt_encode: EncodingKey,
    pub jwt_decode: DecodingKey,
    aes: Aes256Gcm,
}

impl KeyContext {
    pub fn new(key: Option<String>) -> KeyResult<Self> {
        let key = key.unwrap_or_else(|| {
            let key = nanoid!();
            warn!("!!! no sign key provided, use random key: `{key}` !!!");
            warn!("!!! please set SIGN_KEY in .env file or environmental variable to save your login status !!!");
            key
        });
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();

        let aes = Aes256Gcm::new_from_slice(&hash[..]).map_err(|_| KeyError::InvalidData)?;

        let jwt_encode = EncodingKey::from_secret(key.as_bytes());
        let jwt_decode = DecodingKey::from_secret(key.as_bytes());
        Ok(Self {
            jwt_encode,
            jwt_decode,
            aes,
        })
    }

    pub fn sign_jwt(&self, user: &Claims) -> String {
        encode(&Header::default(), user, &self.jwt_encode).expect("encode JWT error")
    }

    #[allow(unused)]
    pub fn decode_jwt(&self, token: &str) -> KeyResult<Claims> {
        let res = decode::<Claims>(token, &self.jwt_decode, &Validation::default())?;
        Ok(res.claims)
    }

    pub fn encrypt_aes(&self, input: &[u8]) -> KeyResult<Vec<u8>> {
        let rand_data: [u8; 12] = thread_rng().gen();
        let nonce = Nonce::from_slice(&rand_data);

        let mut encrypted = self
            .aes
            .encrypt(nonce, input)
            .map_err(|_| KeyError::InvalidData)?;
        encrypted.extend(nonce);

        Ok(encrypted)
    }

    pub fn encrypt_aes_base64(&self, input: &[u8]) -> KeyResult<String> {
        let encrypted = self.encrypt_aes(input)?;
        Ok(URL_SAFE_ENGINE.encode(encrypted))
    }

    pub fn decrypt_aes(&self, input: Vec<u8>) -> KeyResult<Vec<u8>> {
        if input.len() < 12 {
            return Err(KeyError::InvalidData);
        }
        let (content, nonce) = input.split_at(input.len() - 12);

        let Some(nonce) = nonce.try_into().ok() else {
            return Err(KeyError::InvalidData);
        };

        self.aes
            .decrypt(nonce, content)
            .map_err(|_| KeyError::InvalidData)
    }

    pub fn decrypt_aes_base64(&self, input: String) -> KeyResult<Vec<u8>> {
        let input = URL_SAFE_ENGINE.decode(input)?;
        self.decrypt_aes(input)
    }
}

pub struct Endpoint {
    endpoint: String,
    password: Option<String>,
    http_client: Client,
}

impl Endpoint {
    fn new() -> Self {
        Self {
            endpoint: {
                let mut endpoint = dotenvy::var("GOOGLE_ENDPOINT").unwrap_or_default();
                if endpoint.is_empty() {
                    endpoint.push_str("www.googleapis.com");
                }
                endpoint
            },
            password: dotenvy::var("GOOGLE_ENDPOINT_PASSWORD").ok(),
            http_client: Client::new(),
        }
    }

    #[inline]
    fn endpoint(&self) -> String {
        format!(
            "https://{}/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com",
            &self.endpoint
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

#[derive(Debug, Error)]
pub enum FirebaseAuthError {
    #[error("missing cache-control header")]
    Header,
    #[error("missing public key")]
    MissingPubKey,
    #[error("cannot find decoding key")]
    NotFound,
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error(transparent)]
    DecodePEM(#[from] NomErr<PEMError>),
    #[error(transparent)]
    DecodeX509(#[from] NomErr<X509Error>),
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

    async fn init_from_firebase(&mut self, expires_in: Duration) -> Result<(), FirebaseAuthError> {
        let resp = self.endpoint.connect().send().await?;

        let cache = resp
            .headers()
            .typed_get::<CacheControl>()
            .ok_or(FirebaseAuthError::Header)?;
        let now = Utc::now().naive_utc();
        let expires = now
            + cache
                .max_age()
                .and_then(|d| Duration::from_std(d).ok())
                .unwrap_or(expires_in);

        let body = resp.json::<HashMap<String, String>>().await?;

        let mut pub_key = HashMap::new();

        for (key, value) in body {
            let (_, pem) = parse_x509_pem(value.as_bytes())?;
            let cert = pem.parse_x509()?;

            let pbk = encode_pem(&Pem {
                tag: String::from("PUBLIC KEY"),
                contents: cert.public_key().raw.to_vec(),
            });
            let decode = DecodingKey::from_rsa_pem(pbk.as_bytes())?;

            pub_key.insert(key, decode);
        }

        self.expires = expires;
        self.pub_key = pub_key;

        Ok(())
    }

    pub async fn decode_google_token(
        &mut self,
        token: String,
        expires_in: Duration,
    ) -> Result<FirebaseClaims, FirebaseAuthError> {
        let header = decode_header(&token)?;

        if self.expires < Utc::now().naive_utc() {
            self.init_from_firebase(expires_in).await?;
        }

        let kid = header.kid.ok_or(FirebaseAuthError::MissingPubKey)?;

        let key = self.pub_key.get(&kid).ok_or(FirebaseAuthError::NotFound)?;

        let mut validation = Validation::new(header.alg);

        validation.set_audience(&self.project_ids);

        let token = decode(&token, key, &validation)?;

        Ok(token.claims)
    }
}
