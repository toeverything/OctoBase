use super::*;
use byteorder::WriteBytesExt;
use nom::combinator::map;

#[derive(Debug, Clone, PartialEq)]
enum MessageType {
    Auth,
    Awareness,
    AwarenessQuery,
    Doc,
    Custom(u64),
}

fn read_sync_tag(input: &[u8]) -> IResult<&[u8], MessageType> {
    let (tail, tag) = map(read_var_u64, |tag| match tag {
        0 => MessageType::Doc,
        1 => MessageType::Awareness,
        2 => MessageType::Auth,
        3 => MessageType::AwarenessQuery,
        tag => MessageType::Custom(tag),
    })(input)?;

    Ok((tail, tag))
}

fn write_sync_tag<W: Write>(buffer: &mut W, tag: MessageType) -> Result<(), IoError> {
    let tag: u64 = match tag {
        MessageType::Doc => 0,
        MessageType::Awareness => 1,
        MessageType::Auth => 2,
        MessageType::AwarenessQuery => 3,
        MessageType::Custom(tag) => tag,
    };

    write_var_u64(buffer, tag)?;

    Ok(())
}

// sync message
#[derive(Debug, PartialEq)]
pub enum SyncMessage {
    Auth(Option<String>),
    // Awareness(AwarenessMessage),
    Awareness(Vec<u8>),
    AwarenessQuery,
    Doc(DocMessage),
    Custom(u8, Vec<u8>),
}

pub fn read_sync_message(input: &[u8]) -> IResult<&[u8], SyncMessage> {
    let (tail, tag) = read_sync_tag(input)?;

    let (tail, message) = match tag {
        MessageType::Doc => {
            let (tail, doc) = read_doc_message(tail)?;
            (tail, SyncMessage::Doc(doc))
        }
        MessageType::Awareness => {
            let (tail, update) = read_var_buffer(tail)?;
            // TODO: decode awareness update
            (tail, SyncMessage::Awareness(update.into()))
        }
        MessageType::Auth => {
            let (tail, success) = read_var_u64(tail)?;

            if success == 1 {
                (tail, SyncMessage::Auth(None))
            } else {
                let (tail, reason) = read_var_string(tail)?;
                (tail, SyncMessage::Auth(Some(reason)))
            }
        }
        MessageType::AwarenessQuery => (tail, SyncMessage::AwarenessQuery),
        MessageType::Custom(tag) => {
            let (tail, payload) = read_var_buffer(tail)?;
            (tail, SyncMessage::Custom(tag as u8, payload.into()))
        }
    };

    Ok((tail, message))
}

pub fn write_sync_message<W: Write>(buffer: &mut W, msg: &SyncMessage) -> Result<(), IoError> {
    match msg {
        SyncMessage::Auth(reason) => {
            const PERMISSION_DENIED: u8 = 0;
            const PERMISSION_GRANTED: u8 = 1;

            write_sync_tag(buffer, MessageType::Auth)?;
            if let Some(reason) = reason {
                buffer.write_u8(PERMISSION_DENIED)?;
                write_var_string(buffer, reason.into())?;
            } else {
                buffer.write_u8(PERMISSION_GRANTED)?;
            }
        }
        SyncMessage::AwarenessQuery => {
            write_sync_tag(buffer, MessageType::AwarenessQuery)?;
        }
        SyncMessage::Awareness(update) => {
            write_sync_tag(buffer, MessageType::Awareness)?;
            write_var_buffer(buffer, update)?;
        }
        SyncMessage::Doc(doc) => {
            write_sync_tag(buffer, MessageType::Doc)?;
            write_doc_message(buffer, doc)?;
        }
        SyncMessage::Custom(tag, data) => {
            write_var_u64(buffer, *tag as u64)?;
            write_var_buffer(buffer, data)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_tag() {
        let messages = [
            MessageType::Auth,
            MessageType::Awareness,
            MessageType::AwarenessQuery,
            MessageType::Doc,
            MessageType::Custom(128),
        ];

        for msg in messages {
            let mut buffer = Vec::new();

            write_sync_tag(&mut buffer, msg.clone()).unwrap();
            let (tail, decoded) = read_sync_tag(&buffer).unwrap();

            assert_eq!(tail.len(), 0);
            assert_eq!(decoded, msg);
        }
    }

    #[test]
    fn test_sync_message() {
        let messages = [
            SyncMessage::Auth(Some("reason".to_string())),
            SyncMessage::Awareness(vec![1, 2, 3]),
            SyncMessage::AwarenessQuery,
            SyncMessage::Doc(DocMessage::Step1(vec![4, 5, 6])),
            SyncMessage::Doc(DocMessage::Step2(vec![7, 8, 9])),
            SyncMessage::Doc(DocMessage::Update(vec![10, 11, 12])),
            SyncMessage::Custom(13, vec![14, 15, 16]),
        ];

        for msg in messages {
            let mut buffer = Vec::new();
            write_sync_message(&mut buffer, &msg).unwrap();
            let (tail, decoded) = read_sync_message(&buffer).unwrap();
            assert_eq!(tail.len(), 0);
            assert_eq!(decoded, msg);
        }
    }
}
