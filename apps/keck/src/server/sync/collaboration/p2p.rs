use super::{broadcast::*, *};
use dashmap::DashMap;
use jwst::Workspace;
use libp2p::PeerId;
use std::{convert::TryInto, sync::Mutex};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    StateVector,
};

pub struct CollaborationServer {
    broadcast: Mutex<UpdateBroadcast>,
    workspaces: DashMap<String, Arc<Mutex<Workspace>>>,
    state_vectors: DashMap<PeerId, StateVector>,
}

impl CollaborationServer {
    pub fn new() -> CollaborationResult<Self> {
        let broadcast = UpdateBroadcast::new()?;
        Ok(Self {
            broadcast: Mutex::new(broadcast),
            workspaces: DashMap::new(),
            state_vectors: DashMap::new(),
        })
    }

    pub fn listen(&self, port: usize) -> CollaborationResult<()> {
        self.broadcast.lock().unwrap().listen(
            format!("/ip4/0.0.0.0/tcp/{port}/ws")
                .parse()
                .expect("Failed to parse listen addr"),
        )?;
        Ok(())
    }

    fn subscribe_topic<S>(&self, workspace: S) -> CollaborationResult<()>
    where
        S: ToString,
    {
        let mut broadcast = self.broadcast.lock().unwrap();
        broadcast.subscribe(SubscribeTopic::OnFullUpdate(workspace.to_string()).to_string())?;
        broadcast.subscribe(SubscribeTopic::OnStateVector(workspace.to_string()).to_string())?;
        broadcast.subscribe(SubscribeTopic::OnUpdate(workspace.to_string()).to_string())?;
        broadcast
            .subscribe(SubscribeTopic::OnAwarenessUpdate(workspace.to_string()).to_string())?;
        Ok(())
    }

    pub fn add_workspace(&self, workspace: Arc<Mutex<Workspace>>) -> CollaborationResult<()> {
        let id = workspace.lock().unwrap().id();

        self.workspaces.insert(id.clone(), workspace);
        self.subscribe_topic(id)?;
        Ok(())
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
                    return Some(workspace.encode_state_as_update(&sv));
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
                    &workspace.state_vector().encode_v1(),
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
            let mut broadcast = self.broadcast.lock().unwrap();

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
                                            let mut workspace = workspace.lock().unwrap();
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
                                            let workspace = workspace.lock().unwrap();
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
                                            let mut workspace = workspace.lock().unwrap();
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
