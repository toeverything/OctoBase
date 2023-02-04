use super::*;
use axum::{
    body::BoxBody,
    http::{Response, StatusCode, Uri},
};

pub async fn docs_handler(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    info!("get {:?}", uri);
    let res = get_static_file(uri.clone(), files::DOCS_DIST_PATH.to_owned()).await?;

    if res.status() != StatusCode::NOT_FOUND {
        // asset file found
        return Ok(res);
    }

    if let "/index.html" = uri.path() {
        let found_index = get_static_file(
            Uri::from_static("/index.html"),
            files::DOCS_DIST_PATH.to_owned(),
        )
        .await?;
        if found_index.status() == StatusCode::NOT_FOUND {
            return Ok(files::create_404_page(format!(
                "Couldn't find main index file for docs. {GENERAL_INDEX_FILES_ERROR}"
            )));
        }

        return Ok(found_index);
    }

    Ok(create_404_page(format!(
        "Failed to find index file or asset (<code>.{uri}</code>) in <code>{}</code>. {}",
        files::DOCS_DIST_PATH,
        files::GENERAL_INDEX_FILES_ERROR
    )))
}
