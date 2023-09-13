pub mod block;
pub mod schema;
pub mod workspace;

pub use block::{delete_block, get_block, insert_block_children, remove_block_children, set_block};
use schema::InsertChildren;
pub use schema::SubscribeWorkspace;
pub use workspace::{delete_workspace, get_workspace, set_workspace, subscribe_workspace, workspace_client};

use super::*;

fn block_apis(router: Router) -> Router {
    let children_apis = Router::new()
        .route(
            "/children",
            get(block::get_block_children).post(block::insert_block_children),
        )
        .route("/children/:children", delete(block::remove_block_children));

    let block_apis = Router::new().route(
        "/",
        get(block::get_block).post(block::set_block).delete(block::delete_block),
    );

    doc_apis(router)
        .nest("/block/:workspace/:block/", children_apis)
        .nest("/block/:workspace/:block/", block_apis.clone())
        .nest("/block/:workspace/:block", block_apis)
}

fn workspace_apis(router: Router) -> Router {
    router
        .route("/block/:workspace/client", get(workspace::workspace_client))
        .route("/block/:workspace/clients", get(workspace::workspace_clients))
        .route("/block/:workspace/history", get(workspace::history_workspace))
        .route(
            "/block/:workspace",
            get(workspace::get_workspace)
                .post(workspace::set_workspace)
                .delete(workspace::delete_workspace),
        )
        .route("/block/:workspace/flavour/:flavour", get(block::get_block_by_flavour))
        .route("/block/:workspace/blocks", get(workspace::get_workspace_block))
        // .route("/search/:workspace", get(workspace::workspace_search))
        // .route(
        //     "/search/:workspace/index",
        //     get(workspace::get_search_index).post(workspace::set_search_index),
        // )
        .route("/subscribe", post(subscribe_workspace))
}

pub fn blocks_apis(router: Router) -> Router {
    workspace_apis(block_apis(router))
}

#[cfg(test)]
mod tests {
    use axum_test_helper::TestClient;
    use serde_json::{from_str, json, Value};

    use super::*;

    #[tokio::test]
    async fn test_doc_apis() {
        let client = TestClient::new(doc_apis(Router::new()));

        // basic workspace apis
        let resp = client.get("/jwst.json").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        let text = resp.text().await;
        assert!(from_str::<Value>(text.as_str()).is_ok());

        let resp = client.get("/docs/").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_workspace_apis() {
        let client = Arc::new(reqwest::Client::builder().no_proxy().build().unwrap());

        let hook_endpoint = Arc::new(RwLock::new(String::new()));

        let ctx = Arc::new(
            Context::new(
                JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                    .await
                    .ok(),
            )
            .await,
        );
        let client = TestClient::new(
            workspace_apis(Router::new())
                .layer(Extension(ctx.clone()))
                .layer(Extension(client.clone()))
                .layer(Extension(hook_endpoint.clone())),
        );

        // basic workspace apis
        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let resp = client.post("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client.get("/block/test/client").send().await;
        assert_eq!(
            resp.text().await.parse::<u64>().unwrap(),
            ctx.storage.get_workspace("test").await.unwrap().client_id()
        );
        // let resp = client.get("/block/test/history").send().await;
        // assert_eq!(resp.json::<Vec<u64>>().await, Vec::<u64>::new());
        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client.delete("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // workspace history apis
        let resp = client.post("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);

        // let resp = client.get("/search/test/index").send().await;
        // assert_eq!(resp.status(), StatusCode::OK);
        // let index = resp.json::<Vec<String>>().await;
        // assert_eq!(index, vec!["title".to_owned(), "text".to_owned()]);

        // let body = to_string(&json!(["test"])).unwrap();
        // let resp = client
        //     .post("/search/test/index")
        //     .header("content-type", "application/json")
        //     .body(body)
        //     .send()
        //     .await;
        // assert_eq!(resp.status(), StatusCode::OK);

        // let resp = client.get("/search/test/index").send().await;
        // assert_eq!(resp.status(), StatusCode::OK);
        // let index = resp.json::<Vec<String>>().await;
        // assert_eq!(index, vec!["test".to_owned()]);

        let body = json!({
            "hookEndpoint": "localhost:3000/api/hook"
        })
        .to_string();
        let resp = client
            .post("/subscribe")
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
