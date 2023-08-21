use super::*;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::{
    net::TcpStream,
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_tungstenite::{
    tungstenite::{Error as SocketError, Message as WebSocketMessage},
    MaybeTlsStream, WebSocketStream,
};

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

impl From<Message> for WebSocketMessage {
    fn from(value: Message) -> Self {
        match value {
            Message::Binary(data) => WebSocketMessage::Binary(data),
            Message::Close => WebSocketMessage::Close(None),
            Message::Ping => WebSocketMessage::Ping(vec![]),
        }
    }
}

pub fn tungstenite_socket_connector(socket: WebSocket, workspace_id: &str) -> (Sender<Message>, Receiver<Vec<u8>>) {
    let (mut socket_tx, mut socket_rx) = socket.split();

    // send to remote pipeline
    let (local_sender, mut local_receiver) = channel::<Message>(100);
    {
        // socket send thread
        let workspace_id = workspace_id.to_owned();
        tokio::spawn(async move {
            let mut retry = 5;
            while let Some(msg) = local_receiver.recv().await {
                if let Err(e) = socket_tx.send(msg.into()).await {
                    let error = e.to_string();
                    if matches!(e, SocketError::ConnectionClosed | SocketError::AlreadyClosed) || retry == 0 {
                        break;
                    } else {
                        retry -= 1;
                        error!("socket send error: {}", error);
                    }
                } else {
                    retry = 5;
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

    (local_sender, remote_receiver)
}
