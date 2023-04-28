use super::*;
use nom::combinator::map_opt;

pub enum BinaryMessage {
    Sync,
    Awareness,
    Auth,
    AwarenessQuery,
}

fn read_sync_tag(input: &[u8]) -> IResult<&[u8], BinaryMessage> {
    let (tail, tag) = map_opt(read_var_u64, |tag| match tag {
        0 => Some(BinaryMessage::Sync),
        1 => Some(BinaryMessage::Awareness),
        2 => Some(BinaryMessage::Auth),
        3 => Some(BinaryMessage::AwarenessQuery),
        _ => None,
    })(input)?;

    Ok((tail, tag))
}

// doc sync message
pub enum DocMessage {
    // state vector
    Step1(Vec<u8>),
    // update
    Step2(Vec<u8>),
    // update
    Update(Vec<u8>),
}

fn read_doc_message(input: &[u8]) -> IResult<&[u8], DocMessage> {
    let (tail, step) = read_var_u64(input)?;

    match step {
        0 => {
            let (tail, sv) = read_var_buffer(input)?;
            // TODO: decode state vector
            Ok((tail, DocMessage::Step1(sv.into())))
        }
        1 => {
            let (tail, update) = read_var_buffer(input)?;
            // TODO: decode update
            Ok((tail, DocMessage::Step2(update.into())))
        }
        2 => {
            let (tail, update) = read_var_buffer(input)?;
            // TODO: decode update
            Ok((tail, DocMessage::Update(update.into())))
        }
        _ => Err(nom::Err::Error(Error::new(input, ErrorKind::Tag))),
    }
}

pub fn read_sync_message(input: &[u8]) -> IResult<&[u8], SyncMessage> {
    let (tail, tag) = read_sync_tag(input)?;

    let (tail, message) = match tag {
        BinaryMessage::Sync => {
            let (tail, doc) = read_doc_message(tail)?;
            (tail, SyncMessage::Doc(doc))
        }
        BinaryMessage::Awareness => {
            let (tail, update) = read_var_buffer(tail)?;
            // TODO: decode awareness update
            (tail, SyncMessage::Awareness(update.into()))
        }
        BinaryMessage::Auth => unimplemented!("auth message"),
        BinaryMessage::AwarenessQuery => (tail, SyncMessage::AwarenessQuery),
    };

    Ok((tail, message))
}

// sync message
pub enum SyncMessage {
    Auth(Option<String>),
    // Awareness(AwarenessMessage),
    Awareness(Vec<u8>),
    AwarenessQuery,
    Doc(DocMessage),
    Custom(u8, Vec<u8>),
}

impl SyncMessage {
    // fn encode(&self, msg: SyncMessage) -> Vec<u8>;
    fn decode(&self, data: &[u8]) -> JwstRpcResult<SyncMessage> {
        let (tail, message) = read_sync_message(data).map_err(|e| e.map_input(|i| i.len()))?;
        if !tail.is_empty() {
            return Err(JwstRpcError::ProtocolDecode(nom::Err::Error(Error::new(
                tail.len(),
                ErrorKind::Eof,
            ))));
        }
        Ok(message)
    }
}
