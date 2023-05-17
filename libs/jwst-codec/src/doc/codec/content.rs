use super::*;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
pub enum YType {
    Array,
    Map,
    Text,
    XmlElement(String),
    XmlText,
    XmlFragment,
    XmlHook(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Deleted(u64),
    JSON(Vec<Option<String>>),
    Binary(Vec<u8>),
    String(String),
    Embed(JsonValue),
    Format { key: String, value: JsonValue },
    Type(YType),
    Any(Vec<Any>),
    Doc { guid: String, opts: Vec<Any> },
}

impl Content {
    pub(crate) fn from<R: CrdtReader>(decoder: &mut R, tag_type: u8) -> JwstCodecResult<Self> {
        match tag_type {
            1 => Ok(Self::Deleted(decoder.read_var_u64()?)), // Deleted
            2 => {
                let len = decoder.read_var_u64()?;
                let strings = (0..len)
                    .map(|_| {
                        decoder
                            .read_var_string()
                            .map(|s| (s != "undefined").then_some(s))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Self::JSON(strings))
            } // JSON
            3 => Ok(Self::Binary(decoder.read_var_buffer()?.to_vec())), // Binary
            4 => Ok(Self::String(decoder.read_var_string()?)), // String
            5 => {
                let string = decoder.read_var_string()?;
                let json = serde_json::from_str(&string)
                    .map_err(|_| JwstCodecError::DamagedDocumentJson)?;

                Ok(Self::Embed(json))
            } // Embed
            6 => {
                let key = decoder.read_var_string()?;
                let value = decoder.read_var_string()?;
                let value = serde_json::from_str(&value)
                    .map_err(|_| JwstCodecError::DamagedDocumentJson)?;

                Ok(Self::Format { key, value })
            } // Format
            7 => {
                let type_ref = decoder.read_var_u64()?;
                let ytype = match type_ref {
                    0 => YType::Array,
                    1 => YType::Map,
                    2 => YType::Text,
                    3 => YType::XmlElement(decoder.read_var_string()?),
                    4 => YType::XmlFragment,
                    5 => YType::XmlHook(decoder.read_var_string()?),
                    6 => YType::XmlText,
                    _ => {
                        return Err(JwstCodecError::IncompleteDocument(
                            "Unknown y type".to_string(),
                        ))
                    }
                };
                Ok(Self::Type(ytype))
            } // YType
            8 => Ok(Self::Any(Any::from_multiple(decoder)?)), // Any
            9 => {
                let guid = decoder.read_var_string()?;
                let opts = Any::from_multiple(decoder)?;
                Ok(Self::Doc { guid, opts })
            } // Doc
            _ => {
                return Err(JwstCodecError::IncompleteDocument(
                    "Unknown content type".to_string(),
                ))
            }
        }
    }

    pub fn clock_len(&self) -> u64 {
        match self {
            Self::Deleted(len) => *len,
            Self::JSON(strings) => strings.len() as u64,
            Self::String(string) => string.len() as u64,
            Self::Any(any) => any.len() as u64,
            Self::Binary(_)
            | Self::Embed(_)
            | Self::Format { .. }
            | Self::Type(_)
            | Self::Doc { .. } => 1,
        }
    }

    pub fn split(&self, diff: u64) -> JwstCodecResult<(Self, Self)> {
        // TODO: implement split for other types
        match self {
            Self::String(str) => {
                let (left, right) = str.split_at(diff as usize);
                Ok((
                    Self::String(left.to_string()),
                    Self::String(right.to_string()),
                ))
            }
            _ => Err(JwstCodecError::ContentSplitNotSupport(diff)),
        }
    }
}
