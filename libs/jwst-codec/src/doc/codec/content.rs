use super::*;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum Content {
    Deleted(u64),
    JSON(Vec<Option<String>>),
    Binary(Vec<u8>),
    String(String),
    #[cfg_attr(test, proptest(skip))]
    Embed(JsonValue),
    #[cfg_attr(test, proptest(skip))]
    Format {
        key: String,
        value: JsonValue,
    },
    Type(YType),
    Any(Vec<Any>),
    Doc {
        guid: String,
        opts: Vec<Any>,
    },
}

impl Content {
    pub(crate) fn read<R: CrdtReader>(decoder: &mut R, tag_type: u8) -> JwstCodecResult<Self> {
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
                    type_ref => {
                        return Err(JwstCodecError::IncompleteDocument(format!(
                            "Unknown y type: {type_ref}"
                        )))
                    }
                };
                Ok(Self::Type(ytype))
            } // YType
            8 => Ok(Self::Any(Any::read_multiple(decoder)?)), // Any
            9 => {
                let guid = decoder.read_var_string()?;
                let opts = Any::read_multiple(decoder)?;
                Ok(Self::Doc { guid, opts })
            } // Doc
            tag_type => Err(JwstCodecError::IncompleteDocument(format!(
                "Unknown content type: {tag_type}"
            ))),
        }
    }

    pub(crate) fn get_info(&self) -> u8 {
        match self {
            Self::Deleted(_) => 1,
            Self::JSON(_) => 2,
            Self::Binary(_) => 3,
            Self::String(_) => 4,
            Self::Embed(_) => 5,
            Self::Format { .. } => 6,
            Self::Type(_) => 7,
            Self::Any(_) => 8,
            Self::Doc { .. } => 9,
        }
    }

    pub(crate) fn write<W: CrdtWriter>(&self, encoder: &mut W) -> JwstCodecResult {
        match self {
            Self::Deleted(len) => {
                encoder.write_var_u64(*len)?;
            }
            Self::JSON(strings) => {
                encoder.write_var_u64(strings.len() as u64)?;
                for string in strings {
                    match string {
                        Some(string) => encoder.write_var_string(string)?,
                        None => encoder.write_var_string("undefined")?,
                    }
                }
            }
            Self::Binary(buffer) => {
                encoder.write_var_buffer(buffer)?;
            }
            Self::String(string) => {
                encoder.write_var_string(string)?;
            }
            Self::Embed(json) => {
                encoder.write_var_string(json.to_string())?;
            }
            Self::Format { key, value } => {
                encoder.write_var_string(key)?;
                encoder.write_var_string(value.to_string())?;
            }
            Self::Type(ytype) => match ytype {
                YType::Array => encoder.write_var_u64(0)?,
                YType::Map => encoder.write_var_u64(1)?,
                YType::Text => encoder.write_var_u64(2)?,
                YType::XmlElement(string) => {
                    encoder.write_var_u64(3)?;
                    encoder.write_var_string(string)?;
                }
                YType::XmlFragment => encoder.write_var_u64(4)?,
                YType::XmlHook(string) => {
                    encoder.write_var_u64(5)?;
                    encoder.write_var_string(string)?;
                }
                YType::XmlText => encoder.write_var_u64(6)?,
            },
            Self::Any(any) => {
                Any::write_multiple(encoder, any)?;
            }
            Self::Doc { guid, opts } => {
                encoder.write_var_string(guid)?;
                Any::write_multiple(encoder, opts)?;
            }
        }
        Ok(())
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

    pub fn countable(&self) -> bool {
        !matches!(self, Content::Format { .. } | Content::Deleted(_))
    }

    pub fn splittable(&self) -> bool {
        matches!(
            self,
            Self::String { .. } | Self::Any { .. } | Self::JSON { .. }
        )
    }

    // [diff + 1, len - 1)
    pub fn split(&mut self, diff: u64) -> JwstCodecResult<Self> {
        match self {
            Self::String(str) => {
                let (left, right) = Self::split_as_utf16_str(str.as_str(), diff);
                let right = right.to_string();
                *str = left.to_string();
                Ok(Self::String(right))
            }
            Self::JSON(vec) => Ok(Self::JSON(vec.split_off((diff + 1) as usize))),
            Self::Any(vec) => Ok(Self::Any(vec.split_off((diff + 1) as usize))),
            _ => Err(JwstCodecError::ContentSplitNotSupport(diff)),
        }
    }

    /// consider `offset` as a utf-16 encoded string offset
    fn split_as_utf16_str(s: &str, offset: u64) -> (&str, &str) {
        let mut utf_16_offset = 0;
        let mut utf_8_offset = 0;
        for ch in s.chars() {
            utf_16_offset += ch.len_utf16();
            utf_8_offset += ch.len_utf8();
            if utf_16_offset as u64 >= offset {
                break;
            }
        }

        s.split_at(utf_8_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{collection::vec, prelude::*};
    use serde_json::Value as JsonValue;

    fn content_round_trip(content: &Content) -> JwstCodecResult {
        let mut writer = RawEncoder::default();
        writer.write_u8(content.get_info())?;
        content.write(&mut writer)?;

        let mut reader = RawDecoder::new(writer.into_inner());
        let tag_type = reader.read_u8()?;
        assert_eq!(Content::read(&mut reader, tag_type)?, *content);

        Ok(())
    }

    #[test]
    fn test_content() {
        let contents = [
            Content::Deleted(42),
            Content::JSON(vec![
                None,
                Some("test_1".to_string()),
                Some("test_2".to_string()),
            ]),
            Content::Binary(vec![1, 2, 3]),
            Content::String("hello".to_string()),
            Content::Embed(JsonValue::Bool(true)),
            Content::Format {
                key: "key".to_string(),
                value: JsonValue::Number(42.into()),
            },
            Content::Type(YType::Array),
            Content::Type(YType::Map),
            Content::Type(YType::Text),
            Content::Type(YType::XmlElement("test".to_string())),
            Content::Type(YType::XmlFragment),
            Content::Type(YType::XmlHook("test".to_string())),
            Content::Type(YType::XmlText),
            Content::Any(vec![Any::BigInt64(42), Any::String("Test Any".to_string())]),
            Content::Doc {
                guid: "my_guid".to_string(),
                opts: vec![Any::BigInt64(42), Any::String("Test Doc".to_string())],
            },
        ];

        for content in &contents {
            content_round_trip(content).unwrap();
        }
    }

    proptest! {
        #[test]
        fn test_random_content(contents in vec(any::<Content>(), 0..10)) {
            for content in &contents {
                content_round_trip(content).unwrap();
            }
        }
    }
}
