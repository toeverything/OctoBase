use std::sync::Arc;

use axum::body::Bytes;
use http::{Request, Response, StatusCode};
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::DecodingKey;
use tower_http::auth::{AuthorizeRequest, RequireAuthorizationLayer};

use crate::model::Claims;

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
