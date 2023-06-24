use super::{pages::*, *};
use url_escape::decode;

pub async fn fetch_static_response(
    uri: Uri,
    sap: bool,
    fetcher: Option<StaticFileFetcher>,
) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // default page
    if path.is_empty() || path == INDEX_HTML {
        return default_page(fetcher, uri);
    }

    match fetcher.and_then(|fetcher| fetcher(path).or_else(|| fetcher(&decode(path)))) {
        Some(content) => {
            let body = boxed(Full::from(content.data));

            Response::builder()
                .header(CONTENT_TYPE, content.metadata.mimetype())
                .body(body)
                .unwrap()
        }
        None => default_page(
            if !path.contains('.') && sap {
                // return index if no extension and in sap mode
                // fallback to not_found if no fetcher is provided
                fetcher
            } else {
                None
            },
            uri,
        ),
    }
}
