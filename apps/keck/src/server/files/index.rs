use super::*;
use axum::{
    body::{boxed, BoxBody, Empty},
    http::{Response, StatusCode, Uri},
};

pub async fn index_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);

    let res = get_static_file(uri.clone(), INDEX_DIST_PATH.to_owned()).await?;
    if res.status() == StatusCode::NOT_FOUND {
        let res =
            get_static_file(Uri::from_static("/index.html"), INDEX_DIST_PATH.to_owned()).await?;
        if res.status() == StatusCode::NOT_FOUND {
            return if cfg!(debug_assertions) {
                Ok(create_error_page(uri))
            } else {
                Ok(Response::builder()
                    .status(404)
                    .body(boxed(Empty::default()))
                    .unwrap())
            };
        }
        return Ok(res);
    }

    Ok(res)
}
