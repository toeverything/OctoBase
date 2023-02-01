mod broadcast;

use broadcast::{UpdateBroadcast, UpdateBroadcastEvent};
use std::error::Error;
use tokio::io::{self, AsyncBufReadExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut broadcast = UpdateBroadcast::new()?;
    broadcast.subscribe("keck/test/OnUpdate")?;
    // broadcast.listen("/ip4/0.0.0.0/tcp/0/ws".parse()?)?;
    broadcast.connect("/ip4/127.0.0.1/tcp/1280/ws".parse()?)?;

    let mut stdin = io::BufReader::new(io::stdin()).lines();
    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Err(e) = broadcast
                    .publish("keck/test/OnUpdate", line.ok().and_then(|l|l).expect("Stdin not to close")) {
                    println!("Publish error: {e:?}");
                }
            },
            event = broadcast.next() => match event {
                UpdateBroadcastEvent::Message {
                    peer_id, message, ..
                } => {
                    println!("{peer_id}: '{}'", String::from_utf8_lossy(&message.data));
                }
                UpdateBroadcastEvent::Other(other) => {
                    println!("{other}");
                }
            }
        }
    }
}
