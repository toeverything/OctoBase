use jwst_codec::{AwarenessState, DocMessage, SyncMessage};
use y_sync::sync::{Message as YMessage, SyncMessage as YSyncMessage};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    StateVector,
};

pub fn to_sync_message(msg: YMessage) -> Option<SyncMessage> {
    match msg {
        YMessage::Auth(reason) => Some(SyncMessage::Auth(reason)),
        YMessage::Awareness(awareness) => Some(SyncMessage::Awareness(
            awareness
                .clients
                .into_iter()
                .map(|(client_id, state)| (client_id, AwarenessState::new(state.clock as u64, state.json)))
                .collect(),
        )),
        YMessage::AwarenessQuery => Some(SyncMessage::AwarenessQuery),
        YMessage::Sync(doc) => Some(SyncMessage::Doc(match doc {
            YSyncMessage::SyncStep1(update) => DocMessage::Step1(update.encode_v1().unwrap()),
            YSyncMessage::SyncStep2(update) => DocMessage::Step2(update),
            YSyncMessage::Update(update) => DocMessage::Update(update),
        })),
        YMessage::Custom(_tag, _data) => None,
    }
}

pub fn to_y_message(msg: SyncMessage) -> YMessage {
    match msg {
        SyncMessage::Auth(reason) => YMessage::Auth(reason),
        SyncMessage::Awareness(awareness) => YMessage::Awareness(y_sync::awareness::AwarenessUpdate {
            clients: awareness
                .into_iter()
                .map(|(client_id, state)| {
                    (
                        client_id,
                        y_sync::awareness::AwarenessUpdateEntry {
                            clock: state.clock() as u32,
                            json: state.content().into(),
                        },
                    )
                })
                .collect(),
        }),
        SyncMessage::AwarenessQuery => YMessage::AwarenessQuery,
        SyncMessage::Doc(doc) => YMessage::Sync(match doc {
            DocMessage::Step1(update) => YSyncMessage::SyncStep1(StateVector::decode_v1(&update).unwrap()),
            DocMessage::Step2(update) => YSyncMessage::SyncStep2(update),
            DocMessage::Update(update) => YSyncMessage::Update(update),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use jwst_codec::{read_sync_message, write_sync_message, SyncMessageScanner};
    use proptest::{collection::vec, prelude::*};
    use yrs::updates::{
        decoder::DecoderV1,
        encoder::{Encoder, EncoderV1},
    };

    use super::*;

    #[test]
    fn test_sync_message_compatibility() {
        let messages = [
            SyncMessage::Auth(Some("reason".to_string())),
            SyncMessage::Awareness(HashMap::from([(1, AwarenessState::new(1, "test".into()))])),
            SyncMessage::AwarenessQuery,
            SyncMessage::Doc(DocMessage::Step1(vec![1, 2, 3])),
            SyncMessage::Doc(DocMessage::Step2(vec![7, 8, 9])),
            SyncMessage::Doc(DocMessage::Update(vec![10, 11, 12])),
        ];

        for msg in messages {
            let mut buffer = Vec::new();
            write_sync_message(&mut buffer, &msg).unwrap();

            {
                // check messages encode are compatible
                let mut decoder = DecoderV1::from(buffer.as_slice());
                let new_msg = YMessage::decode(&mut decoder).unwrap();
                if let Some(new_msg) = to_sync_message(new_msg) {
                    assert_eq!(new_msg, msg);
                }
            }

            {
                // check messages decode are compatible
                let mut encoder = EncoderV1::new();
                to_y_message(msg.clone()).encode(&mut encoder).unwrap();

                let buffer = encoder.to_vec();
                let (tail, decoded) = read_sync_message(&buffer).unwrap();
                assert_eq!(tail.len(), 0);
                assert_eq!(decoded, msg);
            }
        }
    }

    #[derive(Debug, Clone)]
    struct WrappedAwarenessState(AwarenessState);

    impl Arbitrary for WrappedAwarenessState {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            // clock in yrs only can parse as u32
            any::<(u32, String)>()
                .prop_map(|(c, s)| WrappedAwarenessState(AwarenessState::new(c as u64, s)))
                .boxed()
        }
    }

    fn arbitrary_sync_message() -> impl Strategy<Value = SyncMessage> {
        prop_oneof![
            Just(SyncMessage::Auth(None)),
            any::<String>().prop_map(|s| SyncMessage::Auth(Some(s))),
            any::<HashMap<u64, WrappedAwarenessState>>()
                .prop_map(|s| SyncMessage::Awareness(s.into_iter().map(|(k, v)| (k, v.0)).collect())),
            Just(SyncMessage::AwarenessQuery),
            // binary in step1 must be a valid state vector in yrs, but in y-octo, you can parse it after parse the
            // message any::<Vec<u8>>().prop_map(|s| SyncMessage::Doc(DocMessage::Step1(s))),
            any::<Vec<u8>>().prop_map(|s| SyncMessage::Doc(DocMessage::Step2(s))),
            any::<Vec<u8>>().prop_map(|s| SyncMessage::Doc(DocMessage::Update(s))),
        ]
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_sync_message_scanner_compatibility(yocto in vec(arbitrary_sync_message(), 0..10)) {
            let mut buffer = Vec::new();

            for message in &yocto {
                write_sync_message(&mut buffer, message).unwrap();
            }

            let result: Result<Vec<SyncMessage>, _> = SyncMessageScanner::new(&buffer).collect();
            assert_eq!(result.unwrap(), yocto);

            {
                let mut decoder = DecoderV1::from(buffer.as_slice());
                let yrs: Result<Vec<YMessage>, _> = y_sync::sync::MessageReader::new(&mut decoder).collect();
                let yrs = yrs.unwrap().into_iter().filter_map(to_sync_message).collect::<Vec<_>>();
                assert_eq!(yrs, yocto);
            }
        }
    }
}
