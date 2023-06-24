use super::{broadcast::*, *};
use dashmap::DashMap;
use futures::executor::block_on;
use jwst::Workspace;
use libp2p::PeerId;
use std::convert::TryInto;
use tokio::sync::Mutex;
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    StateVector,
};

pub struct CollaborationServer {
    broadcast: Arc<Mutex<UpdateBroadcast>>,
    publish: Arc<Mutex<UpdateBroadcast>>,
    state_vectors: DashMap<PeerId, StateVector>,
    workspaces: DashMap<String, Arc<Mutex<Workspace>>>,
}

unsafe impl Send for CollaborationServer {}

impl CollaborationServer {
    pub fn new() -> CollaborationResult<Self> {
        let broadcast = UpdateBroadcast::new()?;
        let publish = UpdateBroadcast::new()?;
        Ok(Self {
            broadcast: Arc::new(Mutex::new(broadcast)),
            publish: Arc::new(Mutex::new(publish)),
            state_vectors: DashMap::new(),
            workspaces: DashMap::new(),
        })
    }

    pub async fn connect(&self, port: usize) -> CollaborationResult<()> {
        self.broadcast.lock().await.connect(
            format!("/ip4/127.0.0.1/tcp/{port}/ws")
                .parse()
                .expect("Failed to parse listen addr"),
        )?;
        self.publish.lock().await.connect(
            format!("/ip4/127.0.0.1/tcp/{port}/ws")
                .parse()
                .expect("Failed to parse listen addr"),
        )?;
        Ok(())
    }

    pub async fn listen(&self, port: usize) -> CollaborationResult<()> {
        self.broadcast.lock().await.listen(
            format!("/ip4/0.0.0.0/tcp/{port}/ws")
                .parse()
                .expect("Failed to parse listen addr"),
        )?;
        self.publish.lock().await.connect(
            format!("/ip4/127.0.0.1/tcp/{port}/ws")
                .parse()
                .expect("Failed to parse listen addr"),
        )?;
        Ok(())
    }

    async fn subscribe_topic<S>(
        &self,
        id: S,
        workspace: Arc<Mutex<Workspace>>,
    ) -> CollaborationResult<()>
    where
        S: ToString,
    {
        let on_sv_update = SubscribeTopic::OnStateVector(id.to_string()).to_string();
        let on_update = SubscribeTopic::OnUpdate(id.to_string()).to_string();

        let mut broadcast = self.broadcast.lock().await;
        broadcast.subscribe(SubscribeTopic::OnFullUpdate(id.to_string()).to_string())?;
        broadcast.subscribe(&on_sv_update)?;
        broadcast.subscribe(&on_update)?;
        broadcast.subscribe(SubscribeTopic::OnAwarenessUpdate(id.to_string()).to_string())?;

        let publish = self.publish.clone();
        let mut ws = workspace.lock().await;
        let workspace = workspace.clone();
        let observe = ws.observe(move |_, event| {
            let update = event.update.clone();
            debug!("update: {update:?}");
            let (mut publish, workspace) = block_on(async {
                let publish = publish.lock().await;
                let workspace = workspace.lock().await;

                (publish, workspace)
            });

            if let Err(e) =
                publish.publish(&on_sv_update, workspace.state_vector().encode_v1().unwrap())
            {
                log::error!("Failed to publish state vector: {}", e);
            }
            if let Err(e) = publish.publish(&on_update, update) {
                log::error!("Failed to publish update: {}", e);
            }
        });
        // FIXME: Subscription is not designed to be able to access safely across threads. We need to fix this problem after upgrading yrs0.14.
        std::mem::forget(observe);
        Ok(())
    }

    pub async fn add_workspace(&self, workspace: Arc<Mutex<Workspace>>) -> CollaborationResult<()> {
        let id = workspace.lock().await.id();

        self.workspaces.insert(id.clone(), workspace.clone());
        self.subscribe_topic(id, workspace).await?;
        Ok(())
    }

    pub async fn publish_update<S: ToString>(&self, id: S, update: Vec<u8>) {
        let mut publish = self.publish.lock().await;
        if let Err(e) =
            publish.publish(SubscribeTopic::OnUpdate(id.to_string()).to_string(), update)
        {
            log::error!("Failed to publish update: {}", e);
        }
    }

    fn apply_state_vector(
        &self,
        peer_id: PeerId,
        update: &[u8],
        workspace: &Workspace,
    ) -> Option<Vec<u8>> {
        match StateVector::decode_v1(update) {
            Ok(sv) => {
                self.state_vectors.insert(peer_id, sv.clone());
                if sv != workspace.state_vector() {
                    return Some(workspace.encode_state_as_update(&sv).unwrap());
                }
            }
            Err(err) => {
                warn!("Failed to decode state vector from {peer_id}: {}", err);
            }
        }
        return None;
    }

    fn apply_update(
        &self,
        update: &[u8],
        workspace: &mut Workspace,
        broadcast: &mut UpdateBroadcast,
    ) {
        match workspace.sync_apply_update(update) {
            Ok(data) => {
                self.apply_state_vector(
                    broadcast.peer_id(),
                    &workspace.state_vector().encode_v1().unwrap(),
                    &workspace,
                );
                if let Err(e) =
                    broadcast.publish(SubscribeTopic::OnUpdate(workspace.id()).to_string(), data)
                {
                    warn!("Failed to publish update: {}", e);
                }
            }
            Err(err) => {
                warn!("Failed to apply update: {}", err);
            }
        }
    }

    pub async fn serve(&self) -> CollaborationResult<()> {
        loop {
            let mut broadcast = self.broadcast.lock().await;

            tokio::select! {
                event = broadcast.next() => {
                    match event {
                        UpdateBroadcastEvent::Message {
                            peer_id, message, ..
                        } => {
                            if let Some(topic) = broadcast.find_topic(&message.topic) {
                                match topic.as_str().try_into() {
                                    Ok(SubscribeTopic::OnFullUpdate(workspace)) => {
                                        debug!("{} OnFullUpdate: {:?}", peer_id, message.data);
                                        if let Some(workspace) = self.workspaces.get(&workspace) {
                                            let mut workspace = workspace.lock().await;
                                            self.apply_update(
                                                &message.data,
                                                &mut workspace,
                                                &mut broadcast,
                                            );
                                        }
                                    }
                                    Ok(SubscribeTopic::OnStateVector(workspace)) => {
                                        debug!("{workspace} OnStateVector from {peer_id}: {:?}", message.data);
                                        if let Some(workspace) = self.workspaces.get(&workspace) {
                                            let workspace = workspace.lock().await;
                                            if let Some(update) = self.apply_state_vector(peer_id, &message.data, &workspace) {
                                                if let Err(e) = broadcast.publish(SubscribeTopic::OnUpdate(workspace.id()).to_string(), update) {
                                                    warn!("Failed to publish update: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Ok(SubscribeTopic::OnUpdate(workspace)) => {
                                        debug!("{} OnUpdate: {:?}", peer_id, message.data);
                                        if let Some(workspace) = self.workspaces.get(&workspace) {
                                            let mut workspace = workspace.lock().await;
                                            self.apply_update(
                                                &message.data,
                                                &mut workspace,
                                                &mut broadcast,
                                            );
                                        }
                                    }
                                    Ok(SubscribeTopic::OnAwarenessUpdate(workspace)) => {}
                                    _ => warn!("{:?}", topic),
                                }
                            } else {
                                warn!("Unknown topic: {:?}", message.topic);
                            }
                        }
                        UpdateBroadcastEvent::Subscribed { peer_id, topic } => {
                            debug!("{} Subscribed {}", peer_id, topic);
                        }
                        UpdateBroadcastEvent::Unsubscribed { peer_id, topic } => {
                            debug!("{} Unsubscribed {}", peer_id, topic);
                        }
                        UpdateBroadcastEvent::ConnectionEstablished {
                            peer_id,
                            concurrent_dial_errors,
                            ..
                        } => {
                            debug!(
                                "{peer_id} ConnectionEstablished: {}",
                                if concurrent_dial_errors.len() > 0 {
                                    concurrent_dial_errors.join(",")
                                } else {
                                    "success".to_string()
                                }
                            );
                        }
                        UpdateBroadcastEvent::ConnectionClosed { peer_id, cause, .. } => {
                            if let Some(cause) = cause {
                                warn!("{} ConnectionClosed: {}", peer_id, cause);
                            } else {
                                debug!("{} ConnectionClosed", peer_id);
                            }
                        }
                        UpdateBroadcastEvent::Other(other) => {
                            debug!("{other:?}");
                        }
                    }
                }
                _ = shutdown_signal() => {
                    return Ok(())
                }
            }
        }
    }
}
