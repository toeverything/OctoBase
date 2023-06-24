use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jwst_logger::{instrument, tracing};

///  Health check.
/// - Return 204 No Content.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api",
    path = "/healthz",
    responses(
        (status = 204, description = "Healthy")
    )
)]
#[instrument]
pub async fn health_check() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod test {
    use super::{
        super::{make_rest_route, Context},
        *,
    };
    use axum::Extension;
    use axum_test_helper::TestClient;
    use cloud_database::CloudDatabase;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_health_check() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let resp = client.get("/healthz").send().await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }
}
