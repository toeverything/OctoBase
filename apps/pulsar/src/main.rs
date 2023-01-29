use futures::prelude::*;
use libp2p::{
    core, dns,
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, IdentTopic as Topic,
        MessageAuthenticity, MessageId, ValidationMode,
    },
    identity, mplex, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, websocket, PeerId, Swarm, Transport,
};
use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
    time::Duration,
};
use tokio::io::{self, AsyncBufReadExt};

pub fn ws_transport(
    keypair: identity::Keypair,
) -> std::io::Result<core::transport::Boxed<(PeerId, core::muxing::StreamMuxerBox)>> {
    let transport = websocket::WsConfig::new(dns::TokioDnsConfig::system(
        tcp::tokio::Transport::new(tcp::Config::new().nodelay(true)),
    )?);

    Ok(transport
        .upgrade(core::upgrade::Version::V1)
        .authenticate(noise::NoiseAuthenticated::xx(&keypair).unwrap())
        .multiplex(mplex::MplexConfig::default())
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

#[derive(NetworkBehaviour)]
struct WebSocketBehaviour {
    gossipsub: Gossipsub,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {local_peer_id}");

    // Set up an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = ws_transport(local_key.clone())?;

    // To content-address message, we can take the hash of message and use it as an ID.
    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    // Set a custom gossipsub configuration
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .expect("Valid config");

    // build a gossipsub network behaviour
    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config)
        .expect("Correct configuration");

    // Create a Gossipsub topic
    let topic = Topic::new("test-net");

    // subscribes to our topic
    gossipsub.subscribe(&topic)?;

    // Create a Swarm to manage peers and events
    let mut swarm = {
        // let mdns = mdns::tokio::Behaviour::new(mdns::Config::default())?;
        let behaviour = WebSocketBehaviour { gossipsub };
        Swarm::with_tokio_executor(transport, behaviour, local_peer_id)
    };

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/tcp/1281/ws".parse()?)?;

    // swarm.dial("/dns4/mbp14/tcp/1281/ws".parse::<Multiaddr>().unwrap())?;

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // Kick it off
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.expect("Stdin not to close").expect("Stdin not to close").as_bytes()) {
                    println!("Publish error: {e:?}");
                }
            },
            event = swarm.select_next_some() => match event {
                // SwarmEvent::Behaviour(WebSocketBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                //     for (peer_id, _multiaddr) in list {
                //         println!("mDNS discovered a new peer: {peer_id}");
                //         swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                //     }
                // },
                // SwarmEvent::Behaviour(WebSocketBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                //     for (peer_id, _multiaddr) in list {
                //         println!("mDNS discover peer has expired: {peer_id}");
                //         swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                //     }
                // },
                SwarmEvent::Behaviour(WebSocketBehaviourEvent::Gossipsub(GossipsubEvent::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => println!(
                        "Got message: '{}' with id: {id} from peer: {peer_id}",
                        String::from_utf8_lossy(&message.data),
                    ),
                other => {
                    println!("{other:?}");
                }
            }
        }
    }
}
