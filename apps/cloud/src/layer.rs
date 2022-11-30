use std::sync::Arc;

use axum::body::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::DecodingKey;
use tower_http::{
    auth::{AuthorizeRequest, RequireAuthorizationLayer},
    cors::{Any, CorsLayer},
};

use crate::utils::decode_jwt;

pub fn make_cors_layer() -> CorsLayer {
    let origins = [
        "http://localhost:4200".parse().unwrap(),
        "http://127.0.0.1:4200".parse().unwrap(),
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
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

#[derive(Clone)]
pub struct Auth {
    pub_key: Arc<DecodingKey>,
}

impl<B> AuthorizeRequest<B> for Auth {
    type ResponseBody = UnsyncBoxBody<Bytes, axum::Error>;

    fn authorize(&mut self, request: &mut Request<B>) -> Result<(), Response<Self::ResponseBody>> {
        if let Some(claims) = request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| decode_jwt(header, &self.pub_key).ok())
        {
            request.extensions_mut().insert(Arc::new(claims));

            Ok(())
        } else {
            let unauthorized_response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(UnsyncBoxBody::default())
                .unwrap();

            Err(unauthorized_response)
        }
    }
}

pub fn make_auth_layer() -> RequireAuthorizationLayer<Auth> {
    let pub_key = dotenvy::var("PUBLIC_KEY").expect("should provide public key");
    let pub_key = DecodingKey::from_rsa_pem(pub_key.as_bytes()).expect("decode public key error");

    RequireAuthorizationLayer::custom(Auth {
        pub_key: Arc::new(pub_key),
    })
}
