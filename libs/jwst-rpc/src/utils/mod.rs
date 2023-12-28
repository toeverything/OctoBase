mod memory_workspace;
mod server_context;

use std::io::Write;

use jwst_codec::{CrdtReader, CrdtWriter, JwstCodecError, JwstCodecResult, RawDecoder, RawEncoder};
pub use memory_workspace::connect_memory_workspace;
pub use server_context::MinimumServerContext;

use super::*;

pub fn encode_update_with_guid<S: AsRef<str>>(update: &[u8], guid: S) -> JwstCodecResult<Vec<u8>> {
    let mut encoder = RawEncoder::default();
    encoder.write_var_string(guid)?;
    let mut buffer = encoder.into_inner();

    buffer
        .write_all(update)
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

pub fn decode_update_with_guid(update: &[u8]) -> JwstCodecResult<(String, &[u8])> {
    let mut decoder = RawDecoder::new(update);
    let guid = decoder.read_var_string()?;
    let update = decoder.drain();

    Ok((guid, update))
}
