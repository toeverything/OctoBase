use super::*;
#[cfg(test)]
use y_sync::sync::Message as YMessage;
#[cfg(test)]
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    StateVector,
};

#[cfg(test)]
pub fn to_sync_message(msg: YMessage) -> SyncMessage {
    match msg {
        YMessage::Auth(reason) => SyncMessage::Auth(reason),
        YMessage::Awareness(awareness) => SyncMessage::Awareness(
            awareness
                .clients
                .into_iter()
                .map(|(client_id, state)| {
                    (
                        client_id,
                        AwarenessState::new(state.clock as u64, state.json),
                    )
                })
                .collect(),
        ),
        YMessage::AwarenessQuery => SyncMessage::AwarenessQuery,
        YMessage::Sync(doc) => SyncMessage::Doc(match doc {
            y_sync::sync::SyncMessage::SyncStep1(update) => {
                DocMessage::Step1(update.encode_v1().unwrap())
            }
            y_sync::sync::SyncMessage::SyncStep2(update) => DocMessage::Step2(update),
            y_sync::sync::SyncMessage::Update(update) => DocMessage::Update(update),
        }),
        YMessage::Custom(tag, data) => SyncMessage::Custom(tag as u8, data),
    }
}

#[cfg(test)]
pub fn to_y_message(msg: SyncMessage) -> YMessage {
    match msg {
        SyncMessage::Auth(reason) => YMessage::Auth(reason),
        SyncMessage::Awareness(awareness) => {
            YMessage::Awareness(y_sync::awareness::AwarenessUpdate {
                clients: awareness
                    .into_iter()
                    .map(|(client_id, state)| {
                        (
                            client_id,
                            y_sync::awareness::AwarenessUpdateEntry {
                                clock: state.clock as u32,
                                json: state.content,
                            },
                        )
                    })
                    .collect(),
            })
        }
        SyncMessage::AwarenessQuery => YMessage::AwarenessQuery,
        SyncMessage::Doc(doc) => YMessage::Sync(match doc {
            DocMessage::Step1(update) => {
                y_sync::sync::SyncMessage::SyncStep1(StateVector::decode_v1(&update).unwrap())
            }
            DocMessage::Step2(update) => y_sync::sync::SyncMessage::SyncStep2(update),
            DocMessage::Update(update) => y_sync::sync::SyncMessage::Update(update),
        }),
        SyncMessage::Custom(tag, data) => YMessage::Custom(tag, data),
    }
}

pub fn convert_awareness_update(update: y_sync::awareness::AwarenessUpdate) -> SyncMessage {
    let states = update
        .clients
        .into_iter()
        .map(|(client_id, state)| {
            (
                client_id,
                AwarenessState::new(state.clock as u64, state.json),
            )
        })
        .collect::<AwarenessStates>();

    SyncMessage::Awareness(states)
}
