use super::*;
use axum::{
    body::{boxed, Body, BoxBody},
    http::{Request, Response, StatusCode, Uri},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

#[cfg(debug_assertions)]
pub static PAGE_404: &str = include_str!("./404-dev.html");
#[cfg(not(debug_assertions))]
pub static PAGE_404: &str = include_str!("./404.html");

pub const INDEX_DIST_PATH: &str = if cfg!(debug_assertions) {
    "./apps/frontend/dist/apps/jwst"
} else {
    "/app/dist"
};

pub const DOCS_DIST_PATH: &str = if cfg!(debug_assertions) {
    "./apps/handbook/book"
} else {
    "/app/book"
};

pub const GENERAL_INDEX_FILES_ERROR: &'static str = if cfg!(debug_assertions) {
    r#"If you aren't already, make sure you are using <code>cargo run --package=keck</code> from the workspace root."#
} else {
    ""
};

pub fn create_404_page(message: impl AsRef<str>) -> Response<BoxBody> {
    Response::builder()
        .header("Content-Type", "text/html")
        .status(404)
        .body(boxed(
            files::PAGE_404.replace("{{ERROR_BANNER}}", message.as_ref()),
        ))
        .unwrap()
}

fn create_home_page() -> Response<BoxBody> {
    Response::builder()
        .header("Content-Type", "text/html")
        .status(200)
        .body(boxed(files::PAGE_404.replace("{{ERROR_BANNER}}", "")))
        .unwrap()
}

pub fn create_error_page(uri: Uri) -> Response<BoxBody> {
    if let "/" | "" = uri.path() {
        create_home_page()
    } else if let "/index.html" = uri.path() {
        create_404_page(format!("Failed to find built index for <code>{INDEX_DIST_PATH}/index.html</code>. Make sure you've run <code>pnpm install && pnpm build</code> in <code>apps/frontend</code> to access this page. {GENERAL_INDEX_FILES_ERROR}"))
    } else {
        create_404_page(format!("Failed to find index file or asset (<code>.{uri}</code>) at <code>{INDEX_DIST_PATH}</code>. {GENERAL_INDEX_FILES_ERROR}"))
    }
}

// Reference from https://benw.is/posts/serving-static-files-with-axum
pub async fn get_static_file(
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
