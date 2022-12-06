use std::{collections::HashMap, sync::Arc};

use axum::body::Bytes;
use chrono::{NaiveDateTime, Utc};
use futures::future::BoxFuture;
use http::{header::CACHE_CONTROL, HeaderMap, Method, Request, Response, StatusCode};
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use reqwest::Client;
use tokio::sync::{RwLock, RwLockReadGuard};
use tower_http::{
    auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer},
    cors::{Any, CorsLayer},
};
use tracing::info;
use x509_parser::prelude::parse_x509_pem;

use crate::{model::Claims, utils::CacheControl};

pub fn make_cors_layer() -> CorsLayer {
    let origins = [
        "http://localhost:4200".parse().unwrap(),
        "http://127.0.0.1:4200".parse().unwrap(),
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
        "http://localhost:5173".parse().unwrap(),
        "http://127.0.0.1:5173".parse().unwrap(),
    ];

    CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // allow requests from any origin
        .allow_origin(origins)
        .allow_headers(Any)
}

struct AuthState {
    expires: NaiveDateTime,
    pub_key: HashMap<String, DecodingKey>,
}

#[derive(Clone)]
pub struct Auth {
    state: Arc<RwLock<AuthState>>,
    http_client: Client,
}

impl Auth {
    async fn init_from_firebase(&self) -> RwLockReadGuard<AuthState> {
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

        let mut state = self.state.write().await;

        state.expires = expires;
        state.pub_key = pub_key;

        state.downgrade()
    }

    async fn decode_firebase_jwt(&self, headers: &HeaderMap) -> Option<Claims> {
        let token = headers.get("Authorization")?.to_str().ok()?;
        let header = decode_header(token).ok()?;
        let state = self.state.read().await;

        let state = if state.expires < Utc::now().naive_utc() {
            drop(state);
            self.init_from_firebase().await
        } else {
            state
        };
        let key = state.pub_key.get(&header.kid?)?;

        match decode::<Claims>(token, key, &Validation::new(header.alg)).map(|d| d.claims) {
            Ok(c) => Some(c),
            Err(e) => {
                info!("invalid token {}", e);
                None
            }
        }
    }
}

impl<B: Send + Sync + 'static> AsyncAuthorizeRequest<B> for Auth {
    type RequestBody = B;
    type ResponseBody = UnsyncBoxBody<Bytes, axum::Error>;

    type Future = BoxFuture<'static, Result<Request<B>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, mut request: Request<B>) -> Self::Future {
        let state = self.clone();
        Box::pin(async move {
            if let Some(claims) = state.decode_firebase_jwt(request.headers()).await {
                request.extensions_mut().insert(Arc::new(claims));

                Ok(request)
            } else {
                let unauthorized_response = Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(UnsyncBoxBody::default())
                    .unwrap();

                Err(unauthorized_response)
            }
        })
    }
}

pub fn make_firebase_auth_layer(http_client: Client) -> AsyncRequireAuthorizationLayer<Auth> {
    AsyncRequireAuthorizationLayer::new(Auth {
        http_client,
        state: Arc::new(RwLock::new(AuthState {
            expires: NaiveDateTime::MIN,
            pub_key: HashMap::new(),
        })),
    })
}
