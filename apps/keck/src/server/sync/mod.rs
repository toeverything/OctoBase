mod blobs;
mod collaboration;

use axum::routing::{get, post, put};

use super::*;

pub fn sync_handler(router: Router) -> Router {
    let router = if cfg!(feature = "api") {
        router
    } else {
        router.nest(
            "/api",
            Router::new()
                .route("/workspace/:id/blob", put(blobs::upload_blob_in_workspace))
                .route("/workspace/:id/blob/:name", get(blobs::get_blob_in_workspace)),
        )
    }
    .nest_service(
        "/collaboration/:workspace",
        post(collaboration::auth_handler).get(collaboration::upgrade_handler),
    );

    #[cfg(feature = "webrtc")]
    {
        router.nest_service("/webrtc-sdp/:workspace", post(collaboration::webrtc_handler))
    }

    #[cfg(not(feature = "webrtc"))]
    router
}
