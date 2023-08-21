use std::io::Write;

use super::*;

pub fn encode_update_with_guid<S: AsRef<str>>(update: Vec<u8>, guid: S) -> JwstCodecResult<Vec<u8>> {
    let mut encoder = RawEncoder::default();
    encoder.write_var_string(guid)?;
    let mut buffer = encoder.into_inner();

    buffer
        .write_all(&update)
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

pub fn decode_update_with_guid(update: Vec<u8>) -> JwstCodecResult<(String, Vec<u8>)> {
    let mut decoder = RawDecoder::new(update);
    let guid = decoder.read_var_string()?;
    let update = decoder.drain();

    Ok((guid, update))
}

pub fn decode_maybe_update_with_guid(binary: Vec<u8>) -> (String, Vec<u8>) {
    if let Ok((guid, update)) = decode_update_with_guid(binary.clone()) {
        (
            guid.clone(),
            // if guid is empty or not ascii, it's not a update with guid
            if guid.is_empty() || !guid.is_ascii() {
                binary
            } else {
                update
            },
        )
    } else {
        ("".into(), binary)
    }
}

pub fn encode_update_as_message(update: Vec<u8>) -> JwstCodecResult<Vec<u8>> {
    let mut buffer = Vec::new();
    write_sync_message(&mut buffer, &SyncMessage::Doc(DocMessage::Update(update)))
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

pub fn merge_updates_v1<V: AsRef<[u8]>, I: IntoIterator<Item = V>>(updates: I) -> JwstCodecResult<Update> {
    let updates = updates
        .into_iter()
        .map(|u| Update::from_ybinary1(u.as_ref().to_vec()))
        .collect::<JwstCodecResult<Vec<_>>>()?;

    Ok(Update::merge(updates))
}
