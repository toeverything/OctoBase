mod broadcast;

use broadcast::{UpdateBroadcast, UpdateBroadcastEvent};
use std::error::Error;
use tokio::io::{self, AsyncBufReadExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut broadcast = UpdateBroadcast::new()?;
    broadcast.subscribe("workspace/general")?;
    // broadcast.listen("/ip4/0.0.0.0/tcp/1281/ws".parse()?)?;
    broadcast.connect("/ip4/127.0.0.1/tcp/1281/ws".parse()?)?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // Kick it off
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Err(e) = broadcast
                    .publish("workspace/general", line.ok().and_then(|l|l).expect("Stdin not to close")) {
                    println!("Publish error: {e:?}");
                }
            },
            event = broadcast.next() => match event {
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
                UpdateBroadcastEvent::Message {
                    peer_id,
                    id,
                    message,
                } => println!(
                    "Got message: '{}' with id: {id} from peer: {peer_id}",
                    String::from_utf8_lossy(&message.data),
                ),
                UpdateBroadcastEvent::Other(other) => {
                    println!("{other}");
                }
            }
        }
    }
}
