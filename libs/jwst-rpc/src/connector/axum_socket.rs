use super::*;
use axum::extract::ws::{Message as WebSocketMessage, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio_tungstenite::tungstenite::Error as SocketError;

impl From<Message> for WebSocketMessage {
    fn from(value: Message) -> Self {
        match value {
            Message::Binary(data) => WebSocketMessage::Binary(data),
            Message::Close => WebSocketMessage::Close(None),
            Message::Ping => WebSocketMessage::Ping(vec![]),
        }
    }
}

pub fn axum_socket_connector(
    socket: WebSocket,
    workspace_id: &str,
) -> (Sender<Message>, Receiver<Vec<u8>>, Sender<bool>) {
    let (mut socket_tx, mut socket_rx) = socket.split();

    // send to remote pipeline
    let (local_sender, mut local_receiver) = channel::<Message>(100);
    {
        // socket send thread
        let workspace_id = workspace_id.to_owned();
        tokio::spawn(async move {
            while let Some(msg) = local_receiver.recv().await {
                if let Err(e) = socket_tx.send(msg.into()).await {
                    let error = e.to_string();
                    if e.into_inner().downcast::<SocketError>().map_or_else(
                        |_| false,
                        |e| matches!(e.as_ref(), SocketError::ConnectionClosed),
                    ) {
                        break;
                    } else {
                        error!("socket send error: {}", error);
                    }
                }
            }
            info!("socket send final: {}", workspace_id);
        });
    }

    let (remote_sender, remote_receiver) = channel::<Vec<u8>>(512);
    {
        // socket recv thread
        let workspace_id = workspace_id.to_owned();
        tokio::spawn(async move {
            while let Some(msg) = socket_rx.next().await {
                if let Ok(WebSocketMessage::Binary(binary)) = msg {
                    trace!("recv from remote: {}bytes", binary.len());
                    if remote_sender.send(binary).await.is_err() {
                        // pipeline was closed
                        break;
                    }
                }
            }
            info!("socket recv final: {}", workspace_id);
        });
    }

    let (first_init_tx, mut first_init_rx) = channel::<bool>(10);
    {
        // init notify thread
        let workspace_id = workspace_id.to_owned();
        tokio::spawn(async move {
            if let Some(true) = first_init_rx.recv().await {
                info!("socket init success: {}", workspace_id);
            } else {
                error!("socket init failed: {}", workspace_id);
            }
        });
    }

    (local_sender, remote_receiver, first_init_tx)
}
