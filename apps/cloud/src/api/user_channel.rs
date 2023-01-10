use super::*;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use dashmap::DashMap;
use dashmap::DashSet;
use futures::{sink::SinkExt, stream::StreamExt};
use jwst_logger::error;
use jwst_storage::{WorkspaceDetail, WorkspaceWithPermission};
use serde::Deserialize;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

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
    workspace_map: DashMap<String, DashSet<i32>>,
    user_map: DashMap<i32, DashSet<String>>,
    channel: DashMap<(i32, String), Sender<Message>>,
}

impl UserChannel {
    pub fn new() -> Self {
        Self {
            workspace_map: DashMap::new(),
            user_map: DashMap::new(),
            channel: DashMap::new(),
        }
    }

    pub fn update_workspace(&self, workspace_id: String, context: Arc<Context>) {
        let users = self.workspace_map.get(&workspace_id);
        if users.is_none() {
            return;
        }
        let users_clone = users.unwrap().clone();
        let context = context.clone();
        tokio::spawn(async move {
            for user in users_clone.iter() {
                let _ = context
                    .user_channel
                    .update(user.clone(), context.clone())
                    .await;
            }
        });
    }

    pub fn update_user(&self, user_id: i32, context: Arc<Context>) {
        tokio::spawn(async move {
            let _ = context
                .user_channel
                .update(user_id.clone(), context.clone())
                .await;
        });
    }

    pub async fn add_user_observe(&self, user_id: i32, context: Arc<Context>) {
        self.remove_user_observe(user_id);
        if let Ok(data) = context.db.get_user_workspaces(user_id).await {
            data.iter().for_each(|item| {
                let mut user_option_set = self.workspace_map.get(&item.workspace.id.to_string());
                if user_option_set.is_none() {
                    self.workspace_map
                        .insert(item.workspace.id.to_string(), DashSet::new());
                    user_option_set = self.workspace_map.get(&item.workspace.id.to_string());
                }
                let user_set = user_option_set.unwrap();
                user_set.insert(user_id);
            });
            let _ = self.update(user_id.clone(), context.clone()).await;
        }
    }

    pub fn remove_user_observe(&self, user_id: i32) {
        let workspace_set = self.user_map.get(&user_id);
        if workspace_set.is_none() {
            return;
        }
        for item in workspace_set.unwrap().iter() {
            let user_option_set = self.workspace_map.get(&item.clone());
            user_option_set.unwrap().remove(&user_id);
        }
        self.user_map.remove(&user_id);
    }

    async fn update(&self, user_id: i32, context: Arc<Context>) {
        let all_workspace_info = self.get_workspace_list(user_id, context).await;
        let message_text = serde_json::to_string(&all_workspace_info).unwrap();

        let channel = self.channel.clone();
        let mut closed = vec![];
        for item in channel.iter() {
            let ((user, id), tx) = item.pair();
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
        for item in closed {
            let _ = &self.channel.remove(&item);
        }
    }

    async fn get_workspace_list(&self, user_id: i32, context: Arc<Context>) -> AllWorkspaceInfo {
        let workspace_list = context.db.get_user_workspaces(user_id).await.unwrap();
        let mut workspace_detail_list: HashMap<String, Option<WorkspaceDetail>> = HashMap::new();
        let mut workspace_metadata_list: HashMap<String, Any> = HashMap::new();
        for item in workspace_list.iter() {
            let workspace_detail = context
                .db
                .get_workspace_by_id(item.workspace.id.clone())
                .await
                .unwrap();
            workspace_detail_list.insert(item.workspace.id.to_string(), workspace_detail);
            let workspace = init_workspace(&context, &item.workspace.id.to_string())
                .await
                .unwrap();
            workspace_metadata_list.insert(
                item.workspace.id.to_string(),
                workspace.lock().await.metadata().to_json(),
            );
        }
        AllWorkspaceInfo {
            ws_list: workspace_list,
            ws_details: workspace_detail_list,
            metadata: workspace_metadata_list,
        }
    }
}

pub async fn global_ws_handler(
    Extension(ctx): Extension<Arc<Context>>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let user: Option<RefreshToken> = base64::decode_engine(token, &URL_SAFE_ENGINE)
        .ok()
        .and_then(|byte| ctx.decrypt_aes(byte))
        .and_then(|data| serde_json::from_slice(&data).ok());

    let user = if let Some(user) = user {
        if let Ok(true) = ctx.db.verify_refresh_token(&user).await {
            Some(user.user_id)
        } else {
            None
        }
    } else {
        None
    };
    ws.protocols(["AFFiNE"])
        .on_upgrade(move |socket| async move { handle_socket(socket, user, ctx.clone()).await })
}

async fn handle_socket(socket: WebSocket, user: Option<i32>, context: Arc<Context>) {
    // TODO check user
    let user_id = user.unwrap_or(0);
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

    let uuid = Uuid::new_v4().to_string();
    context
        .user_channel
        .channel
        .insert((user_id.clone(), uuid.clone()), tx.clone());
    context
        .user_channel
        .add_user_observe(user.unwrap(), context.clone())
        .await;

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Close(_)) = msg {
            break;
        }
    }

    context.user_channel.channel.remove(&(user_id, uuid));
}
