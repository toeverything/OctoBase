use super::*;
use crate::error_status::ErrorStatus;
use axum::response::IntoResponse;
use axum::{
    body::{boxed, Body, BoxBody},
    http::{Request, Response, StatusCode, Uri},
    routing::get,
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

const INDEX_DIST_PATH: &str = if cfg!(debug_assertions) {
    "./apps/frontend/dist/apps/jwst"
} else {
    "/app/dist"
};

async fn index_handler(uri: Uri) -> Result<Response<BoxBody>, Response<BoxBody>> {
    info!("get {:?}", uri);

    let res = get_static_file(uri.clone(), INDEX_DIST_PATH.to_owned()).await?;
    if res.status() == StatusCode::NOT_FOUND {
        let res =
            get_static_file(Uri::from_static("/index.html"), INDEX_DIST_PATH.to_owned()).await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(ErrorStatus::NotFound.into_response());
        }
        return Ok(res);
    }

    Ok(res)
}

// Reference from https://benw.is/posts/serving-static-files-with-axum
async fn get_static_file(uri: Uri, dist: String) -> Result<Response<BoxBody>, Response<BoxBody>> {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = ServeDir::new(dist).oneshot(req);

    match res.await {
        Ok(res) => Ok(res.map(boxed)),
        Err(err) => Err(ErrorStatus::InternalServerFileError(err).into_response()),
    }
}

pub fn static_files(router: Router) -> Router {
    router.fallback_service(get(files::index_handler))
}
