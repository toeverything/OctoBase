use super::*;
use axum::{
    body::{boxed, Body, BoxBody, Empty},
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

async fn index_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);

    let res = get_static_file(uri.clone(), INDEX_DIST_PATH.to_owned()).await?;
    if res.status() == StatusCode::NOT_FOUND {
        let res =
            get_static_file(Uri::from_static("/index.html"), INDEX_DIST_PATH.to_owned()).await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(Response::builder()
                .status(404)
                .body(boxed(Empty::default()))
                .unwrap());
        }
        return Ok(res);
    }

    Ok(res)
}

// Reference from https://benw.is/posts/serving-static-files-with-axum
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

pub fn static_files(router: Router) -> Router {
    router.fallback_service(get(files::index_handler))
}
