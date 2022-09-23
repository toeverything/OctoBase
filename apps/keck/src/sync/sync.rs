pub use yrs::{updates::decoder::Decode, Update};

use yrs::{
    updates::{
        decoder::Decoder,
        encoder::{Encode, Encoder},
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

/// Create a sync step 1 message based on the state of the current shared document.
pub fn write_step1<E: Encoder>(doc: &Doc, encoder: &mut E) {
    let txn = doc.transact();

    encoder.write_var(MSG_SYNC_STEP_1);
    encoder.write_buf(txn.state_vector().encode_v1());
}

pub fn write_step2<E: Encoder>(doc: &Doc, sv: &[u8], encoder: &mut E) {
    let txn = doc.transact();
    let remote_sv = StateVector::decode_v1(sv).unwrap();

    encoder.write_var(MSG_SYNC_STEP_2);
    encoder.write_buf(txn.encode_diff_v1(&remote_sv));
}

pub fn write_update<E: Encoder>(update: &[u8], encoder: &mut E) {
    encoder.write_var(MSG_SYNC_UPDATE);
    encoder.write_buf(update);
}

pub fn read_sync_message<D: Decoder, E: Encoder>(
    doc: &Doc,
    decoder: &mut D,
    encoder: &mut E,
) -> Option<Vec<u8>> {
    let msg_type = decoder.read_var().unwrap();
    match msg_type {
        MSG_SYNC_STEP_1 => {
            read_sync_step1(doc, decoder, encoder);
            None
        }
        MSG_SYNC_STEP_2 => Some(read_sync_step2(doc, decoder)),
        MSG_SYNC_UPDATE => Some(read_update(doc, decoder)),
        other => panic!("Unknown message type: {} to {}", other, doc.client_id),
    }
}

pub fn read_sync_step1<D: Decoder, E: Encoder>(doc: &Doc, decoder: &mut D, encoder: &mut E) {
    write_step2(doc, decoder.read_buf().unwrap(), encoder)
}

pub fn read_sync_step2<D: Decoder>(doc: &Doc, decoder: &mut D) -> Vec<u8> {
    let mut txn = doc.transact();

    let buf = decoder.read_buf().unwrap();
    let update = Update::decode_v1(buf).unwrap();
    txn.apply_update(update);
    buf.to_vec()
}

pub fn read_update<D: Decoder>(doc: &Doc, decoder: &mut D) -> Vec<u8> {
    read_sync_step2(doc, decoder)
}
