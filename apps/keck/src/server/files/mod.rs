mod docs;
mod files;
mod index;

use super::*;
use axum::routing::get;
use files::*;

pub fn static_files(router: Router) -> Router {
    if cfg!(all(not(debug_assertions), not(feature = "affine"))) {
        // Redirect to docs in production until we have a proper home page
        // FIXME: Notice that you can't test this locally because the debug_assertions are used to determine where the built file are too
        // So, after this redirects you locally, it will still be 404, since the files will be expected at a production path like `/app/book`.
        router
            .nest_service("/docs", get(docs::docs_handler))
            .route("/", get(|| async { Redirect::to("/docs/index.html") }))
    } else {
        router.fallback_service(get(index::index_handler))
    }
}
