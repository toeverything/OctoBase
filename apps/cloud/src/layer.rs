use crate::infrastructure::auth::get_claim_from_headers;

use super::{infrastructure::error_status::ErrorStatus, *};
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
use jsonwebtoken::DecodingKey;
use nanoid::nanoid;
use std::sync::Arc;
use tower_http::{
    auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer},
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct Auth {
    decoding_key: DecodingKey,
}

impl Auth {
    fn check_auth<B>(key: DecodingKey, request: &Request<B>) -> Option<Claims> {
        get_claim_from_headers(request.headers(), &key)
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

const EXCLUDED_URIS: [&str; 1] = ["/api/healthz"];

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
                let uri = request.uri();
                if !EXCLUDED_URIS.contains(&uri.path()) {
                    use form_urlencoded::{parse, Serializer};
                    info!(
                        "[HTTP:request_id={}] {:?} {} {}{}",
                        request_id,
                        request.version(),
                        request.method(),
                        uri.path(),
                        &if let Some(query) = uri
                            .query()
                            .map(|q| parse(q.as_bytes())
                                .filter(|i| i.0 != "token")
                                .fold(Serializer::new(String::new()), |mut s, i| {
                                    s.append_pair(&i.0, &i.1);
                                    s
                                })
                                .finish())
                            .and_then(|q| (!q.is_empty()).then_some(q))
                        {
                            format!("?{}", query)
                        } else {
                            "".to_string()
                        },
                    );
                }

                span
            }),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
