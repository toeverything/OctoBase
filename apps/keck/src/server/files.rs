use super::*;
use axum::{
    body::{boxed, Body, BoxBody},
    http::{Request, Response, StatusCode, Uri},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

const INDEX_DIST_PATH: &str = if cfg!(debug_assertions) {
    "./apps/frontend/dist/apps/jwst"
} else {
    "/app/dist"
};

const DOCS_DIST_PATH: &str = if cfg!(debug_assertions) {
    "./apps/handbook/book"
} else {
    "/app/book"
};

pub async fn index_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);
    let res = get_static_file(uri.clone(), INDEX_DIST_PATH.to_owned()).await?;

    if res.status() == StatusCode::NOT_FOUND {
        get_static_file(Uri::from_static("/index.html"), INDEX_DIST_PATH.to_owned()).await
    } else {
        Ok(res)
    }
}

pub async fn docs_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);
    let res = get_static_file(uri.clone(), DOCS_DIST_PATH.to_owned()).await?;

    if res.status() == StatusCode::NOT_FOUND {
        get_static_file(Uri::from_static("/index.html"), DOCS_DIST_PATH.to_owned()).await
    } else {
        Ok(res)
    }
}

async fn get_static_file(
    uri: Uri,
    dist: String,
) -> Result<Response<BoxBody>, (StatusCode, String)> {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = ServeDir::new(dist).oneshot(req);

    match res.await {
        Ok(res) => Ok(res.map(boxed)),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", err),
        )),
    }
}
