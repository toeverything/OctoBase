use super::{debug, info};
use futures::{future::Either, prelude::*};
use libp2p::{
    core::{muxing, transport, upgrade, ConnectedPoint},
    dns,
    gossipsub::{
        error::{PublishError, SubscriptionError},
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, IdentTopic as Topic,
        MessageAuthenticity, MessageId, TopicHash, ValidationMode,
    },
    identity, mplex, noise,
    swarm::{DialError, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, PeerId, Swarm, Transport, TransportError,
};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    time::Duration,
};
use tokio::io;

fn ws_transport(
    keypair: identity::Keypair,
) -> std::io::Result<transport::Boxed<(PeerId, muxing::StreamMuxerBox)>> {
    let transport = {
        let dns_tcp = dns::TokioDnsConfig::system(tcp::tokio::Transport::new(
            tcp::Config::new().nodelay(true),
        ))?;
        let ws_dns_tcp = websocket::WsConfig::new(dns::TokioDnsConfig::system(
            tcp::tokio::Transport::new(tcp::Config::new().nodelay(true)),
        )?);
        ws_dns_tcp.or_transport(dns_tcp)
    };

    Ok(transport
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseAuthenticated::xx(&keypair).unwrap())
        .multiplex(upgrade::SelectUpgrade::new(
            yamux::YamuxConfig::default(),
            mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

#[derive(NetworkBehaviour)]
struct WebSocketBehaviour {
    gossipsub: Gossipsub,
    #[cfg(features = "discover")]
    mdns: libp2p::mdns::tokio::Behaviour,
}

#[cfg(features = "discover")]
type NextBehaviour = Either<libp2p::mdns::Event, UpdateBroadcastEvent>;
#[cfg(not(features = "discover"))]
type NextBehaviour = Either<(), UpdateBroadcastEvent>;

pub struct UpdateBroadcast {
    peer_id: PeerId,
    swarm: Swarm<WebSocketBehaviour>,
    topic_map: HashMap<TopicHash, String>,
}

impl UpdateBroadcast {
    pub fn new() -> Result<Self, io::Error> {
        // Create a random PeerId
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer id: {local_peer_id}");

        // Set up an encrypted DNS-enabled TCP Transport over the Mplex protocol.
        let transport = ws_transport(local_key.clone())?;

        let gossipsub = {
            // Set a custom gossipsub configuration
            let gossipsub_config = GossipsubConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                .message_id_fn(|message: &GossipsubMessage| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    MessageId::from(s.finish().to_string())
                }) // content-address messages. No two messages of the same content will be propagated.
                .build()
                .expect("Valid config");

            Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config)
                .expect("Correct configuration")
        };

        // Create a Swarm to manage peers and events
        let swarm = {
            #[cfg(features = "discover")]
            let mdns = libp2p::mdns::tokio::Behaviour::new(libp2p::mdns::Config::default())?;
            let behaviour = WebSocketBehaviour {
                gossipsub,
                #[cfg(features = "discover")]
                mdns,
            };
            Swarm::with_tokio_executor(transport, behaviour, local_peer_id)
        };

        Ok(Self {
            peer_id: local_peer_id,
            swarm,
            topic_map: HashMap::new(),
        })
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn subscribe<S: Into<String>>(&mut self, topic: S) -> Result<(), SubscriptionError> {
        let topic_name = topic.into();
        let full_topic_name = format!("/pulsar/{}", &topic_name);
        let topic = Topic::new(&full_topic_name);

        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        self.topic_map.insert(topic.hash(), topic_name);

        info!("subscribed to topic: {}", full_topic_name);
        Ok(())
    }

    pub fn unsubscribe<S: Into<String>>(&mut self, topic: S) -> Result<(), PublishError> {
        let topic_name = format!("/pulsar/{}", topic.into());
        let topic = Topic::new(&topic_name);

        self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic)?;
        self.topic_map.remove(&topic.hash());

        info!("unsubscribed from topic: {}", topic_name);
        Ok(())
    }

    pub fn publish<S, D>(&mut self, topic: S, data: D) -> Result<(), PublishError>
    where
        S: Into<String>,
        D: Into<Vec<u8>>,
    {
        let topic = Topic::new(format!("/pulsar/{}", topic.into()));
        self.swarm.behaviour_mut().gossipsub.publish(topic, data)?;
        Ok(())
    }

    pub fn find_topic(&self, hash: &TopicHash) -> Option<String> {
        self.topic_map.get(hash).map(|s| s.clone())
    }

    pub fn listen(&mut self, addr: Multiaddr) -> Result<(), TransportError<io::Error>> {
        info!("sync service listening: {}", addr);
        self.swarm.listen_on(addr)?;
        Ok(())
    }

    pub fn connect(&mut self, addr: Multiaddr) -> Result<(), DialError> {
        self.swarm.dial(addr)?;

        Ok(())
    }

    async fn next_behaviour(&mut self) -> NextBehaviour {
        self.swarm
            .select_next_some()
            .map(|event| match event {
                #[cfg(features = "discover")]
                SwarmEvent::Behaviour(WebSocketBehaviourEvent::Mdns(event)) => Either::Left(event),
                SwarmEvent::Behaviour(WebSocketBehaviourEvent::Gossipsub(
                    GossipsubEvent::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    },
                )) => Either::Right(UpdateBroadcastEvent::Message {
                    peer_id,
                    id,
                    message,
                }),
                SwarmEvent::Behaviour(WebSocketBehaviourEvent::Gossipsub(
                    GossipsubEvent::Subscribed { peer_id, topic },
                )) => Either::Right(UpdateBroadcastEvent::Subscribed { peer_id, topic }),
                SwarmEvent::Behaviour(WebSocketBehaviourEvent::Gossipsub(
                    GossipsubEvent::Unsubscribed { peer_id, topic },
                )) => Either::Right(UpdateBroadcastEvent::Unsubscribed { peer_id, topic }),
                SwarmEvent::ConnectionEstablished {
                    peer_id,
                    endpoint,
                    num_established,
                    concurrent_dial_errors,
                } => Either::Right(UpdateBroadcastEvent::ConnectionEstablished {
                    peer_id,
                    endpoint,
                    num_established: num_established.into(),
                    concurrent_dial_errors: concurrent_dial_errors
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(addr, error)| format!("{error}: {addr}"))
                        .collect(),
                }),
                SwarmEvent::ConnectionClosed {
                    peer_id,
                    endpoint,
                    num_established,
                    cause,
                } => Either::Right(UpdateBroadcastEvent::ConnectionClosed {
                    peer_id,
                    endpoint,
                    num_established,
                    cause: cause.map(|e| e.to_string()),
                }),
                other => Either::Right(UpdateBroadcastEvent::Other(format!("{other:?}").into())),
            })
            .await
    }

    pub async fn next(&mut self) -> UpdateBroadcastEvent {
        loop {
            match self.next_behaviour().await {
                #[allow(unused_variables)]
                Either::Left(event) => {
                    #[cfg(features = "discover")]
                    {
                        let pubsub = &mut self.swarm.behaviour_mut().gossipsub;

                        match event {
                            libp2p::mdns::Event::Discovered(list) => {
                                for (peer_id, _multiaddr) in list {
                                    debug!("mDNS discovered a new peer: {peer_id}");
                                    pubsub.add_explicit_peer(&peer_id);
                                }
                            }
                            libp2p::mdns::Event::Expired(list) => {
                                for (peer_id, _multiaddr) in list {
                                    debug!("mDNS discover peer has expired: {peer_id}");
                                    pubsub.remove_explicit_peer(&peer_id);
                                }
                            }
                        }
                    }
                }
                Either::Right(event) => {
                    break event;
                }
            }
        }
    }
}

pub enum UpdateBroadcastEvent {
    Message {
        /// The peer that forwarded us this message.
        peer_id: PeerId,
        /// The [`MessageId`] of the message. This should be referenced by the application when
        /// validating a message (if required).
        id: MessageId,
        /// The decompressed message itself.
        message: GossipsubMessage,
    },
    Subscribed {
        peer_id: PeerId,
        topic: TopicHash,
    },
    Unsubscribed {
        peer_id: PeerId,
        topic: TopicHash,
    },
    ConnectionEstablished {
        peer_id: PeerId,
        endpoint: ConnectedPoint,
        num_established: u32,
        concurrent_dial_errors: Vec<String>,
    },
    ConnectionClosed {
        peer_id: PeerId,
        endpoint: ConnectedPoint,
        num_established: u32,
        cause: Option<String>,
    },
    Other(String),
}
