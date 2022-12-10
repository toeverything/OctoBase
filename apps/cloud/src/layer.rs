use std::sync::Arc;

use axum::body::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::DecodingKey;
use tower_http::{
    auth::{AuthorizeRequest, RequireAuthorizationLayer},
    cors::{Any, CorsLayer},
};

use crate::model::Claims;

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

#[derive(Clone)]
pub struct Auth {
    decoding_key: DecodingKey,
}

impl<B> AuthorizeRequest<B> for Auth {
    type ResponseBody = UnsyncBoxBody<Bytes, axum::Error>;

    fn authorize(&mut self, request: &mut Request<B>) -> Result<(), Response<Self::ResponseBody>> {
        use jsonwebtoken::{decode, Validation};
        // TODO: some route don't need auth
        if let Some(claims) = request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|token| {
                decode::<Claims>(token, &self.decoding_key, &Validation::default())
                    .map(|d| d.claims)
                    .ok()
            })
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

pub fn make_firebase_auth_layer(decoding_key: DecodingKey) -> RequireAuthorizationLayer<Auth> {
    RequireAuthorizationLayer::custom(Auth { decoding_key })
}
