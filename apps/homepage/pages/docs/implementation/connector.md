# Sync Connector

Sync Connector is an abstract interface for peer-to-peer synchronization protocol, which enables OctoBase to connect and synchronize with another OctoBase application using various bidirectional transmission protocols. This document aims to help developers understand the process of creating a new connector based on a new transmission protocol.

## General requirements

Usually, when we are writing a connector, we need to return a `Local Sender` and a `Remote Receiver`, provided by the following channels, and execute them in separate threads:

1. **Local Channel**: A channel used to receive from local changes and send changes to remote, which is an asynchronous mpsc (multiple producer, single consumer queue).
2. **Remote Channel**: A channel that receives changes from remote and sends them to local thread, which is an asynchronous mpsc (multiple producers single consumer queue).

In this section, we will use the WebSocket implementation as an example to demonstrate how to implement a new connector.

## Implementing a WebSocket connector

### context struct

First, we need to implement a Context struct, which is used to hold the JwstStorage and broadcast group:

```rust
use jwst_rpc::{BroadcastChannels, RpcContextImpl};
use jwst_storage::{JwstStorage, JwstStorageResult};

pub struct Context {
    channel: BroadcastChannels,
    storage: JwstStorage,
}

impl RpcContextImpl<'_> for Context {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}
```

### connect function

Next, we will write a simple connect function that connects the local OctoBase instance with the remote OctoBase instance:

```rust
use jwst_rpc::{handle_connector, RpcContextImpl};

pub async fn connect(ctx: Context, workspace: String, user_id: String) {
    // Create handshake notification
    let first_init_tx = {
        let (tx, mut rx) = channel::<bool>(10);
        tokio::spawn(async move {
            if let Some(true) = first_init_rx.recv().await {
                println!("Handshake complete");
            }
        });
        tx
    };
    // Create sync thread
    handle_connector(ctx.clone(), workspace.clone(), user_id, move || {
        let (tx, rx) = websocket_connector(socket, &workspace);
        (tx, rx, first_init_tx)
    }).await;
}
```

### connector function

Now, we will start implementing the actual connector function. The connector needs to provide a `Local Sender` and a `Remote Receiver` that are used to receive local and remote changes, respectively.
In this example, we use the tokio-tungstenite library as the WebSocket connection library and omit the necessary dependency imports.

We first write the basic function signature and prepare channels for local and remote:

```rust
pub fn websocket_connector(socket: WebSocket, workspace_id: &str) -> (Sender<Message>, Receiver<Vec<u8>>) {
    // split socket as tx and rx
    let (mut socket_tx, mut socket_rx) = socket.split();

    // send to remote pipeline
    let (local_sender, mut local_receiver) = channel::<Message>(100);
    {
        tokio::spawn(async move {
            // todo: send to remote
        });
    }

    // receive from remote pipeline
    let (remote_sender, remote_receiver) = channel::<Vec<u8>>(512);
    {
        tokio::spawn(async move {
            // todo: receive from remote
        });
    }

    (local_sender, remote_receiver)
}
```

### sending and receiving logic

Next, we will write the sending and receiving logic, starting with the sending logic:

```rust
tokio::spawn(async move {
    // Receive messages from local_receiver and send them to remote
    while let Some(msg) = local_receiver.recv().await {
        if let Err(e) = socket_tx.send(msg.into()).await {
            let error = e.to_string();
            // If the connection has been closed, break the loop
            if matches!(e, SocketError::ConnectionClosed) {
                break;
            } else {
                error!("socket send error: {}", error);
            }
        }
    }
});
```

Then, the receiving logic:

```rust
tokio::spawn(async move {
    // Receive messages from remote and send them to local
    while let Some(msg) = socket_rx.next().await {
        if let Ok(WebSocketMessage::Binary(binary)) = msg {
            if remote_sender.send(binary).await.is_err() {
                // If the connection has been closed, break the loop
                break;
            }
        }
    }
});
```

### finalize the connector

Finally, we will finalize the connector function by combining the sending and receiving logic.

The final code will look like this:

```rust
pub fn websocket_connector(socket: WebSocket, workspace_id: &str) -> (Sender<Message>, Receiver<Vec<u8>>) {
    // split socket as tx and rx
    let (mut socket_tx, mut socket_rx) = socket.split();

    // send to remote pipeline
    let (local_sender, mut local_receiver) = channel::<Message>(100);
    {
        tokio::spawn(async move {
            // Receive messages from local_receiver and send them to remote
            while let Some(msg) = local_receiver.recv().await {
                if let Err(e) = socket_tx.send(msg.into()).await {
                    let error = e.to_string();
                    // If the connection has been closed, break the loop
                    if matches!(e, SocketError::ConnectionClosed) {
                        break;
                    } else {
                        error!("socket send error: {}", error);
                    }
                }
            }
        });
    }

    // receive from remote pipeline
    let (remote_sender, remote_receiver) = channel::<Vec<u8>>(512);
    {
        tokio::spawn(async move {
            // Receive messages from remote and send them to local
            while let Some(msg) = socket_rx.next().await {
                if let Ok(WebSocketMessage::Binary(binary)) = msg {
                    if remote_sender.send(binary).await.is_err() {
                        // If the connection has been closed, break the loop
                        break;
                    }
                }
            }
        });
    }

    (local_sender, remote_receiver)
}
```

At this point, we have implemented a WebSocket connector based on tokio-tungstenite. You can find its complete code [here](https://github.com/toeverything/OctoBase/blob/master/libs/jwst-rpc/src/connector/tungstenite_socket.rs).
