pub use yrs::{updates::decoder::Decode, Update};

use lib0::{decoding::Read, encoding::Write};
use yrs::{
    updates::{
        decoder::{Decoder, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
    Doc, StateVector,
};

pub const MSG_SYNC: usize = 0;
pub const MSG_SYNC_STEP_1: usize = 0;
pub const MSG_SYNC_STEP_2: usize = 1;
pub const MSG_SYNC_UPDATE: usize = 2;

pub fn write_sync<E: Encoder>(encoder: &mut E) {
    encoder.write_var(MSG_SYNC);
}

pub fn encode_init_update(doc: &Doc) -> Vec<u8> {
    let mut encoder = EncoderV1::new();
    write_sync(&mut encoder);

    // Create a sync step 1 message based on the state of the current shared document.
    encoder.write_var(MSG_SYNC_STEP_1);
    encoder.write_buf(doc.transact().state_vector().encode_v1());

    encoder.to_vec()
}

pub fn encode_update(update: &[u8]) -> Vec<u8> {
    let mut encoder = EncoderV1::new();

    write_sync(&mut encoder);

    encoder.write_var(MSG_SYNC_UPDATE);
    encoder.write_buf(update);

    encoder.to_vec()
}

pub fn decode_remote_message(doc: &Doc, binary: Vec<u8>) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut encoder = EncoderV1::new();
    let mut decoder = DecoderV1::from(binary.as_slice());

    if decoder.read_var::<u8>().unwrap() == MSG_SYNC as u8 {
        write_sync(&mut encoder);

        // read sync message
        let msg_type = decoder.read_var().unwrap();
        match msg_type {
            MSG_SYNC_STEP_1 => {
                read_sync_step1(doc, &mut decoder, &mut encoder);
                (Some(encoder.to_vec()), None)
            }
            MSG_SYNC_STEP_2 => (None, Some(read_sync_step2(doc, &mut decoder))),
            MSG_SYNC_UPDATE => (None, Some(read_update(doc, &mut decoder))),
            other => panic!("Unknown message type: {} to {}", other, doc.client_id),
        }
    } else {
        (None, None)
    }
}

fn read_sync_step1<D: Decoder, E: Encoder>(doc: &Doc, decoder: &mut D, encoder: &mut E) {
    let remote_sv = StateVector::decode_v1(decoder.read_buf().unwrap()).unwrap();

    encoder.write_var(MSG_SYNC_STEP_2);
    encoder.write_buf(doc.transact().encode_diff_v1(&remote_sv));
}

fn read_sync_step2<D: Decoder>(doc: &Doc, decoder: &mut D) -> Vec<u8> {
    let mut txn = doc.transact();

    let buf = decoder.read_buf().unwrap();
    let update = Update::decode_v1(buf).unwrap();
    txn.apply_update(update);
    buf.to_vec()
}

fn read_update<D: Decoder>(doc: &Doc, decoder: &mut D) -> Vec<u8> {
    read_sync_step2(doc, decoder)
}
