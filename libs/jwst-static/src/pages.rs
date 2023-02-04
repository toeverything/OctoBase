use super::*;

#[cfg(debug_assertions)]
pub static PAGE_404: &str = include_str!("./404-dev.html");
#[cfg(not(debug_assertions))]
pub static PAGE_404: &str = include_str!("./404.html");

pub const INDEX_HTML: &str = "index.html";

pub fn create_404_page(message: impl AsRef<str>) -> Response<BoxBody> {
    let message = message.as_ref();
    Response::builder()
        .header("Content-Type", "text/html")
        .status(if message.len() > 0 { 404 } else { 200 })
        .body(boxed(PAGE_404.replace("{{ERROR_BANNER}}", message)))
        .unwrap()
}

pub fn create_error_page(uri: Uri) -> Response<BoxBody> {
    if let "/" | "" = uri.path() {
        create_404_page("")
    } else if let "/index.html" = uri.path() {
        create_404_page(format!(
            r#"
        Failed to find built index for <code>/index.html</code>.
        Make sure you've run <code>pnpm install && pnpm build</code> in <code>apps/frontend</code> to access this page.
        "#
        ))
    } else {
        create_404_page(format!(
            "Failed to find index file or asset (<code>.{uri}</code>)."
        ))
    }
}

pub fn default_page(fetcher: Option<StaticFileFetcher>, uri: Uri) -> Response {
    fetcher
        .and_then(|fetcher| {
            fetcher(INDEX_HTML).map(|content| {
                let body = boxed(Full::from(content.data));

                Response::builder()
                    .header(CONTENT_TYPE, "text/html")
                    .body(body)
                    .unwrap()
            })
        })
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                create_error_page(uri)
            } else {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(boxed(Full::from("404")))
                    .unwrap()
            }
        })
}
