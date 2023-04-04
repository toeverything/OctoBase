use crate::infrastructure::auth::get_claim_from_token;

use super::*;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use cloud_database::{WorkspaceDetail, WorkspaceWithPermission};
use futures::{sink::SinkExt, stream::StreamExt};
use jwst_logger::error;
use nanoid::nanoid;
use serde::Deserialize;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc::channel, RwLock};

#[derive(Deserialize)]
pub struct Param {
    token: String,
}

#[derive(Serialize)]
pub struct AllWorkspaceInfo {
    ws_list: Vec<WorkspaceWithPermission>,
    ws_details: HashMap<String, Option<WorkspaceDetail>>,
    metadata: HashMap<String, Any>,
}

// pub enum MessageType {
//     Workspaces = 0,
//     WorkspaceDetail = 1,
//     WorkspaceMetadata = 2,
// }

// pub struct UserMessage {
//     ms_type: MessageType,
//     data: Any,
// }

pub struct UserChannel {
    workspace_map: RwLock<HashMap<String, RwLock<HashSet<String>>>>,
    user_map: RwLock<HashMap<String, RwLock<HashSet<String>>>>,
    channel: RwLock<HashMap<(String, String), Sender<Message>>>,
}

impl UserChannel {
    pub fn new() -> Self {
        Self {
            workspace_map: RwLock::new(HashMap::new()),
            user_map: RwLock::new(HashMap::new()),
            channel: RwLock::new(HashMap::new()),
        }
    }

    pub(super) async fn update_workspace(&self, workspace_id: String, context: Arc<Context>) {
        if let Some(users) = self.workspace_map.read().await.get(&workspace_id) {
            for user in users.read().await.iter() {
                context
                    .user_channel
                    .update(user.clone(), context.clone())
                    .await;
            }
        }
    }

    pub fn update_user(&self, user_id: String, context: Arc<Context>) {
        tokio::spawn(async move {
            context
                .user_channel
                .update(user_id.clone(), context.clone())
                .await;
        });
    }

    pub async fn add_user_observe(&self, user_id: String, context: Arc<Context>) {
        self.remove_user_observe(user_id.clone()).await;
        if let Ok(data) = context.db.get_user_workspaces(user_id.clone()).await {
            for item in data {
                let mut workspace_map = self.workspace_map.write().await;
                if let Some(user_set) = workspace_map.get(&item.id.to_string()) {
                    user_set.write().await.insert(user_id.clone());
                } else {
                    workspace_map.insert(
                        item.id.to_string(),
                        RwLock::new(HashSet::from([user_id.clone()])),
                    );
                }
            }

            self.update(user_id.clone(), context.clone()).await;
        }
    }

    async fn remove_user_observe(&self, user_id: String) {
        let mut user_map = self.user_map.write().await;
        if let Some(workspace_set) = user_map.get(&user_id) {
            for item in workspace_set.read().await.iter() {
                if let Some(user_set) = self.workspace_map.read().await.get(&item.clone()) {
                    user_set.write().await.remove(&user_id.clone());
                }
            }
            user_map.remove(&user_id);
        }
    }

    async fn update(&self, user_id: String, context: Arc<Context>) {
        let all_workspace_info = self.get_workspace_list(user_id.clone(), context).await;
        let message_text = serde_json::to_string(&all_workspace_info).unwrap();

        let mut closed = vec![];
        for ((user, id), tx) in self.channel.read().await.iter() {
            if &user_id == user {
                if tx.is_closed() {
                    closed.push((user.clone(), id.clone()));
                } else if let Err(e) = tx.send(Message::Text(message_text.clone())).await {
                    if !tx.is_closed() {
                        error!("on user_channel_update error: {}", e);
                    }
                }
            }
        }

        let mut channel = self.channel.write().await;
        for item in closed {
            channel.remove(&item);
        }
    }

    async fn get_workspace_list(&self, user_id: String, context: Arc<Context>) -> AllWorkspaceInfo {
        let workspace_list = context.db.get_user_workspaces(user_id).await.unwrap();
        let mut workspace_detail_list: HashMap<String, Option<WorkspaceDetail>> = HashMap::new();
        let mut workspace_metadata_list: HashMap<String, Any> = HashMap::new();
        for item in workspace_list.iter() {
            let workspace_detail = context
                .db
                .get_workspace_by_id(item.id.clone())
                .await
                .unwrap();
            workspace_detail_list.insert(item.id.to_string(), workspace_detail);

            match context.storage.get_workspace(item.id.clone()).await {
                Ok(workspace) => {
                    workspace_metadata_list
                        .insert(item.id.to_string(), workspace.metadata().into());
                }
                Err(e) => error!("get workspace {} metadata error: {}", item.id, e),
            }
        }
        AllWorkspaceInfo {
            ws_list: workspace_list,
            ws_details: workspace_detail_list,
            metadata: workspace_metadata_list,
        }
    }
}

#[instrument(skip(context, token))]
pub async fn global_ws_handler(
    Extension(context): Extension<Arc<Context>>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let user = match get_claim_from_token(&token, &context.key.jwt_decode) {
        Some(claims) => Some(claims.user.id),
        None => None,
    };

    ws.protocols(["AFFiNE"])
        .on_upgrade(move |socket| handle_socket(socket, user, context))
}

async fn handle_socket(socket: WebSocket, user: Option<String>, context: Arc<Context>) {
    // TODO check user
    let user_id = user.unwrap_or('0'.to_string());
    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = channel(100);

    {
        // socket thread
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = socket_tx.send(msg).await {
                    error!("send user_channel error: {}", e);
                    break;
                }
            }
        });
    }

    let uuid = nanoid!();
    context
        .user_channel
        .channel
        .write()
        .await
        .insert((user_id.clone(), uuid.clone()), tx.clone());
    context
        .user_channel
        .add_user_observe(user_id.clone(), context.clone())
        .await;

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Close(_)) = msg {
            break;
        }
    }

    context
        .user_channel
        .channel
        .write()
        .await
        .remove(&(user_id, uuid));
}
