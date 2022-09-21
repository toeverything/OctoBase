use super::*;
use axum::extract::Path;

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    info!("get_workspace: {}", workspace);
    utils::init_doc(context.clone(), workspace.clone()).await;

    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        let json = trx.get_map("blocks").to_json().to_string();
        ([(header::CONTENT_TYPE, "application/json")], json).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data")
    )
)]
pub async fn set_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> String {
    info!("set_workspace: {}", workspace);

    utils::init_doc(context.clone(), workspace.clone()).await;

    let doc = context.doc.get(&workspace).unwrap();
    let doc = doc.lock().await;

    doc.transact().get_map("blocks").to_json().to_string()
}
