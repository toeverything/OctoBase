use jwst::{Block, DocStorage, error, Workspace};
use jwst_rpc::start_client;
use jwst_storage::JwstStorage;
use std::collections::hash_map::Entry;
use std::sync::{Arc};
use std::thread::sleep;
use std::time::Duration;
use reqwest::Response;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    let (workspace_id, mut workspace, storage) = rt.block_on(async move {
        let workspace_id = String::from("1");
        // let storage: Arc<JwstStorage> = Arc::new(JwstStorage::new("sqlite::memory:").await.unwrap());
        let storage: Arc<JwstStorage> = Arc::new(JwstStorage::new_with_sqlite("jwst_client").await.unwrap());
        let remote = String::from("ws://localhost:3000/collaboration/1");
        storage
            .create_workspace(workspace_id.clone())
            .await
            .unwrap();

        create_workspace_with_api(workspace_id.clone()).await;

        if let Entry::Vacant(entry) = storage
            .docs()
            .remote()
            .write()
            .await
            .entry(workspace_id.clone())
        {
            let (tx, _rx) = tokio::sync::broadcast::channel(10);
            entry.insert(tx);
        };

        let workspace = start_client(&storage, workspace_id.clone(), remote)
            .await
            .unwrap();

        (workspace_id, workspace, storage)
    });

    let workspace = {
        let id = workspace_id.clone();
        let storage = storage.clone();
        let sub = workspace.observe(move |_, e| {
            let id = id.clone();
            let rt = Runtime::new().unwrap();
            // println!("update in rpc main: {:?}", &e.update);
            if let Err(e) = rt.block_on(async {
                storage.docs().write_update(id, &e.update).await
            }) {
                error!("Failed to write update to storage: {}", e);
                println!("Failed to write update to storage: {}", e);
            }
        });
        std::mem::forget(sub);

        workspace
    };

    let block = create_block(&workspace, "7".to_string(), "list".to_string());
    println!("from main thread, create a block: {:?}", block);
    println!("get block 7 from server: {}", get_block_with_api_sync( workspace_id.clone(), "7".to_string()));

    sleep(Duration::from_secs(4));

    let block = create_block(&workspace, "8".to_string(), "list".to_string());
    println!("from main thread, create a block: {:?}", block);
    println!("get block 8 from server: {}", get_block_with_api_sync( workspace_id.clone(), "8".to_string()));

    sleep(Duration::from_secs(4));

    let block = create_block(&workspace, "9".to_string(), "list".to_string());
    println!("from main thread, create a block: {:?}", block);
    println!("get block 9 from server: {}", get_block_with_api_sync( workspace_id.clone(), "9".to_string()));

    workspace.with_trx(|mut trx| {
        let space = trx.get_space("blocks");
        let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
        println!("blocks from local storage:");
        for block in blocks {
            println!("block: {:?}", block);
        }
    });

    sleep(Duration::from_secs(10));

    println!("------------------after sync------------------");
    println!("get block 7 from server: {}", get_block_with_api_sync(workspace_id.clone(), "7".to_string()));
    println!("get block 8 from server: {}", get_block_with_api_sync(workspace_id.clone(), "8".to_string()));
    println!("get block 9 from server: {}", get_block_with_api_sync( workspace_id.clone(), "9".to_string()));
    workspace.with_trx(|mut trx| {
        let space = trx.get_space("blocks");
        let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
        println!("blocks from local storage:");
        for block in blocks {
            println!("block: {:?}", block);
        }
    });
}

async fn create_workspace_with_api(workspace_id: String) {
    let client = reqwest::Client::new();
    client
        .post(format!("http://localhost:3000/api/block/{}", workspace_id))
        .send()
        .await
        .unwrap();
}

async fn get_block_with_api(workspace_id: String, block_id: String) -> Response {
    let client = reqwest::Client::new();

    client
        .get(format!("http://localhost:3000/api/block/{}/{}", workspace_id, block_id))
        .send()
        .await
        .unwrap()
}

fn get_block_with_api_sync(workspace_id: String, block_id: String) -> String {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let resp = get_block_with_api(workspace_id.clone(), block_id.to_string()).await;
        resp.text().await.unwrap()
    })
}

fn create_block(workspace: &Workspace, block_id: String, block_flavour: String) -> Block {
    workspace.with_trx(|mut trx| {
        let space = trx.get_space("blocks");
        space.create(&mut trx.trx, block_id, block_flavour)
    })
}

