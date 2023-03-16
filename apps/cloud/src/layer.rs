use super::*;
use axum::{
    body::Body,
    http::{Request, Response},
    response::IntoResponse,
    Router,
};
use bytes::Bytes;
use cloud_database::Claims;
use futures_util::future::BoxFuture;
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::{decode, DecodingKey, Validation};
use nanoid::nanoid;
use std::sync::Arc;
use tower_http::{
    auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer},
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};

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

#[derive(Clone, Copy)]
pub struct MakeRequestUuid;

impl MakeRequestId for MakeRequestUuid {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let request_id = nanoid!().parse().unwrap();
        Some(RequestId::new(request_id))
    }
}

pub fn make_tracing_layer(router: Router) -> Router {
    router
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                let request_id = request
                    .extensions()
                    .get::<RequestId>()
                    .and_then(|id| id.header_value().to_str().ok())
                    .unwrap_or_default();
                let span = info_span!(
                    "HTTP",
                    %request_id,
                );
                info!(
                    "[HTTP:request_id={}] {:?} {} {}",
                    request_id,
                    request.version(),
                    request.method(),
                    request.uri(),
                );
                span
            }),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
