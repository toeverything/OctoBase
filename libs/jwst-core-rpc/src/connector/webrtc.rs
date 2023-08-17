use super::Message;
use jwst_core::{debug, error, info, trace, warn};

use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use webrtcrs::{
    api::APIBuilder,
    data_channel::{
        data_channel_init::RTCDataChannelInit, data_channel_message::DataChannelMessage,
        OnMessageHdlrFn,
    },
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
};

const DATA_CHANNEL_ID: u16 = 42;
const DATA_CHANNEL_LABEL: &str = "affine";

fn create_on_message_handler(channel: Sender<Vec<u8>>) -> OnMessageHdlrFn {
    Box::new(move |message: DataChannelMessage| {
        let channel = channel.clone();
        Box::pin(async move {
            let m = message.data.to_vec();
            trace!("WebRTC Recv: {:?}", m.clone());
            channel.send(m).await.unwrap();
        })
    })
}

async fn new_peer_connection() -> (
    Arc<RTCPeerConnection>,
    Sender<Message>,
    Receiver<Vec<u8>>,
    tokio::sync::broadcast::Receiver<bool>,
) {
    let api = APIBuilder::new().build();
    let peer_connection = Arc::new(
        api.new_peer_connection(RTCConfiguration {
            ..Default::default()
        })
        .await
        .unwrap(),
    );

    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        warn!("Client: Peer Connection State has changed: {s}");
        if s == RTCPeerConnectionState::Failed {
            error!("Peer Connection has gone to failed exiting");
        }
        Box::pin(async {})
    }));

    let data_channel = peer_connection
        .create_data_channel(
            DATA_CHANNEL_LABEL,
            Some(RTCDataChannelInit {
                ordered: Some(true),
                negotiated: Some(DATA_CHANNEL_ID),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    let d0 = Arc::clone(&data_channel);
    let (local_sender, mut local_receiver) = channel::<Message>(100);

    let (signal_tx, mut signal_rx) = tokio::sync::broadcast::channel::<bool>(1);
    let signal_tx2 = signal_tx.subscribe();
    let sender_handle = tokio::spawn(async move {
        signal_rx.recv().await.ok();
        while let Some(msg) = local_receiver.recv().await {
            match msg {
                Message::Binary(data) => {
                    trace!("WebRTC Send: {:?}", data.clone());
                    d0.send(&Bytes::copy_from_slice(data.as_slice()))
                        .await
                        .unwrap();
                }
                Message::Close => info!("Close"),
                Message::Ping => info!("Ping"),
            }
        }
    });

    data_channel.on_open(Box::new(move || {
        debug!("Data channel opened");
        Box::pin(async move {
            signal_tx.send(true).ok();
        })
    }));

    data_channel.on_close(Box::new(move || {
        debug!("Data channel closed");
        sender_handle.abort();
        Box::pin(async move {})
    }));

    let (remote_sender, remote_receiver) = channel::<Vec<u8>>(512);
    data_channel.on_message(create_on_message_handler(remote_sender));

    (peer_connection, local_sender, remote_receiver, signal_tx2)
}

pub async fn webrtc_datachannel_client_begin() -> (
    RTCSessionDescription,
    Arc<RTCPeerConnection>,
    Sender<Message>,
    Receiver<Vec<u8>>,
    tokio::sync::broadcast::Receiver<bool>,
) {
    let (peer_connection, tx, rx, s) = new_peer_connection().await;
    let offer = peer_connection.create_offer(None).await.unwrap();

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    match peer_connection.set_local_description(offer).await {
        Ok(_) => {}
        Err(e) => {
            error!("set local description failed: {}", e);
        }
    }

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    let local_desc = peer_connection.local_description().await.unwrap();
    (local_desc, peer_connection, tx, rx, s)
}

pub async fn webrtc_datachannel_client_commit(
    answer: RTCSessionDescription,
    peer_connection: Arc<RTCPeerConnection>,
) {
    match peer_connection.set_remote_description(answer).await {
        Ok(_) => {}
        Err(e) => {
            error!("set remote description failed: {}", e);
        }
    }
}

pub async fn webrtc_datachannel_server_connector(
    offer: RTCSessionDescription,
) -> (
    RTCSessionDescription,
    Sender<Message>,
    Receiver<Vec<u8>>,
    tokio::sync::broadcast::Receiver<bool>,
) {
    let (peer_connection, tx, rx, s) = new_peer_connection().await;

    match peer_connection.set_remote_description(offer).await {
        Ok(_) => {}
        Err(e) => {
            error!("set remote description failed: {}", e);
        }
    }

    let answer = peer_connection.create_answer(None).await.unwrap();

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    match peer_connection.set_local_description(answer).await {
        Ok(_) => {}
        Err(e) => {
            error!("set local description failed: {}", e);
        }
    }

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    let local_desc = peer_connection.local_description().await.unwrap();
    (local_desc, tx, rx, s)
}
