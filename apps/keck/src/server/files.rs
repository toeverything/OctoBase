use super::*;
use axum::{
    body::{boxed, Body, BoxBody, Empty},
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

static GENERAL_INDEX_FILES_ERROR: &'static str = if cfg!(debug_assertions) {
    r#"If you aren't already, make sure you are using <code>cargo run --package=keck</code> from the workspace root."#
} else {
    ""
};

async fn index_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);

    let res = get_static_file(uri.clone(), INDEX_DIST_PATH.to_owned()).await?;
    if res.status() == StatusCode::NOT_FOUND {
        let res =
            get_static_file(Uri::from_static("/index.html"), INDEX_DIST_PATH.to_owned()).await?;
        if res.status() == StatusCode::NOT_FOUND {
            return if cfg!(debug_assertions) {
                if let "/" | "" = uri.path() {
                    Ok(create_homepage())
                } else if let "/index.html" = uri.path() {
                    Ok(create_404(format!("Failed to find built index for <code>{INDEX_DIST_PATH}/index.html</code>. Make sure you've run <code>pnpm install && pnpm build</code> in <code>apps/frontend</code> to access this page. {GENERAL_INDEX_FILES_ERROR}")))
                } else {
                    Ok(create_404(format!("Failed to find index file or asset (<code>.{uri}</code>) at <code>{INDEX_DIST_PATH}</code>. {GENERAL_INDEX_FILES_ERROR}")))
                }
            } else {
                Ok(Response::builder()
                    .status(404)
                    .body(boxed(Empty::default()))
                    .unwrap())
            };
        }
    }

    Ok(res)
}

async fn docs_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);
    let res = get_static_file(uri.clone(), DOCS_DIST_PATH.to_owned()).await?;

    if res.status() != StatusCode::NOT_FOUND {
        // asset file found
        return Ok(res);
    }

    if let "/index.html" = uri.path() {
        let found_index =
            get_static_file(Uri::from_static("/index.html"), DOCS_DIST_PATH.to_owned()).await?;
        if found_index.status() == StatusCode::NOT_FOUND {
            return Ok(create_404(format!(
                "Couldn't find main index file for docs. {GENERAL_INDEX_FILES_ERROR}"
            )));
        }

        return Ok(found_index);
    }

    Ok(create_404(format!(
        "Failed to find index file or asset (<code>.{uri}</code>) in <code>{DOCS_DIST_PATH}</code>. {GENERAL_INDEX_FILES_ERROR}"
    )))
}

#[cfg(debug_assertions)]
static PAGE_404: &str = include_str!("./404-dev.html");
#[cfg(not(debug_assertions))]
static PAGE_404: &str = include_str!("./404.html");

fn create_404(message: impl AsRef<str>) -> Response<BoxBody> {
    Response::builder()
        .header("Content-Type", "text/html")
        .status(404)
        .body(boxed(
            PAGE_404.replace("{{ERROR_BANNER}}", message.as_ref()),
        ))
        .unwrap()
}

fn create_homepage() -> Response<BoxBody> {
    Response::builder()
        .header("Content-Type", "text/html")
        .status(200)
        .body(boxed(PAGE_404.replace("{{ERROR_BANNER}}", "")))
        .unwrap()
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
    if cfg!(all(not(debug_assertions), not(feature = "affine"))) {
        // Redirect to docs in production until we have a proper home page
        // FIXME: Notice that you can't test this locally because the debug_assertions are used to determine where the built file are too
        // So, after this redirects you locally, it will still be 404, since the files will be expected at a production path like `/app/book`.
        router
            .nest_service("/docs", get(files::docs_handler))
            .route("/", get(|| async { Redirect::to("/docs/index.html") }))
    } else {
        router.fallback_service(get(files::index_handler))
    }
}
