use axum::{
    http::{Request, Response},
    response::IntoResponse,
};
use bytes::Bytes;
use cloud_database::Claims;
use futures_util::future::BoxFuture;
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::sync::Arc;
use tower_http::auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer};

use crate::error_status::ErrorStatus;

#[derive(Clone)]
pub struct Auth {
    decoding_key: DecodingKey,
}

impl Auth {
    fn check_auth<B>(key: DecodingKey, request: &Request<B>) -> Option<Claims> {
        request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|token| {
                decode::<Claims>(token, &key, &Validation::default())
                    .map(|d| d.claims)
                    .ok()
            })
    }
}

impl<B> AsyncAuthorizeRequest<B> for Auth
where
    B: Send + Sync + 'static,
{
    type RequestBody = B;
    type ResponseBody = UnsyncBoxBody<Bytes, axum::Error>;
    type Future = BoxFuture<'static, Result<Request<B>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, mut request: Request<B>) -> Self::Future {
        let key = self.decoding_key.clone();
        Box::pin(async move {
            if let Some(claims) = Self::check_auth(key, &request) {
                request.extensions_mut().insert(Arc::new(claims));
                Ok(request)
            } else {
                Err(ErrorStatus::Unauthorized.into_response())
            }
        })
    }
}

pub fn make_firebase_auth_layer(decoding_key: DecodingKey) -> AsyncRequireAuthorizationLayer<Auth> {
    AsyncRequireAuthorizationLayer::new(Auth { decoding_key })
}
