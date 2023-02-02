use super::{broadcast::*, *};
use dashmap::DashMap;
use jwst::Workspace;
use std::{convert::TryInto, sync::Mutex};

pub struct CollaborationServer {
    broadcast: Mutex<UpdateBroadcast>,
    workspaces: DashMap<String, Arc<Mutex<Workspace>>>,
}

impl CollaborationServer {
    pub fn new() -> CollaborationResult<Self> {
        let broadcast = UpdateBroadcast::new()?;
        Ok(Self {
            broadcast: Mutex::new(broadcast),
            workspaces: DashMap::new(),
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
        broadcast.subscribe(SubscribeTopic::StateVector(workspace.to_string()).to_string())?;
        broadcast.subscribe(SubscribeTopic::UpdateWithSV(workspace.to_string()).to_string())?;
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

    pub async fn serve(&self) -> CollaborationResult<()> {
        loop {
            let mut broadcast = self.broadcast.lock().unwrap();

            tokio::select! {
                event = broadcast.next() => {
                    match event {
                        UpdateBroadcastEvent::Message { peer_id, message, .. } => {
                            if let Some(topic) = broadcast.find_topic(&message.topic) {
                                match topic.as_str().try_into() {
                                    Ok(SubscribeTopic::StateVector(workspace)) => {}
                                    Ok(SubscribeTopic::UpdateWithSV(workspace)) => {}
                                    Ok(SubscribeTopic::OnUpdate(workspace)) => {
                                        debug!("{} OnUpdate: {:?}", peer_id, message.data);
                                        if let Some(workspace) = self.workspaces.get(&workspace) {
                                            let mut workspace = workspace.lock().unwrap();
                                            match workspace.sync_apply_update(&message.data) {
                                                Ok(data) => {
                                                    if let Err(e) = broadcast.publish(
                                                        SubscribeTopic::OnUpdate(workspace.id())
                                                            .to_string(),
                                                        data,
                                                    ) {
                                                        warn!("Failed to publish update: {}", e);
                                                    }
                                                }
                                                Err(err) => {
                                                    warn!("Failed to apply update: {}", err);
                                                }
                                            }
                                        }
                                    }
                                    Ok(SubscribeTopic::OnAwarenessUpdate(workspace)) => {}
                                    _ => warn!("{:?}", topic),
                                }
                            } else {
                                warn!("Unknown topic: {:?}", message.topic);
                            }
                        }
                        UpdateBroadcastEvent::Subscribed { peer_id, topic } =>{
                            debug!("{} Subscribed to {}", peer_id, topic);
                        },
                        UpdateBroadcastEvent::Unsubscribed { peer_id, topic } =>{
                            debug!("{} Unsubscribed from {}", peer_id, topic);
                        },
                        UpdateBroadcastEvent::ConnectionEstablished { peer_id, concurrent_dial_errors, .. } =>{
                            debug!("{peer_id} ConnectionEstablished: {}", if concurrent_dial_errors.len() > 0 {
                                concurrent_dial_errors.join(",")
                            } else {
                                "success".to_string()
                            });
                        },
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
