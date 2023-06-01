use std::ops::Deref;

use super::*;
use serde_json::Value as JsonValue;

#[derive(Clone)]
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
    #[cfg_attr(test, proptest(skip))]
    Type(YTypeRef),
    Any(Vec<Any>),
    Doc {
        guid: String,
        opts: Vec<Any>,
    },
}

impl PartialEq for Content {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Deleted(len1), Self::Deleted(len2)) => len1 == len2,
            (Self::JSON(vec1), Self::JSON(vec2)) => vec1 == vec2,
            (Self::Binary(vec1), Self::Binary(vec2)) => vec1 == vec2,
            (Self::String(str1), Self::String(str2)) => str1 == str2,
            (Self::Embed(json1), Self::Embed(json2)) => json1 == json2,
            (
                Self::Format {
                    key: key1,
                    value: value1,
                },
                Self::Format {
                    key: key2,
                    value: value2,
                },
            ) => key1 == key2 && value1 == value2,
            (Self::Any(any1), Self::Any(any2)) => any1 == any2,
            (Self::Doc { guid: guid1, .. }, Self::Doc { guid: guid2, .. }) => guid1 == guid2,
            (Self::Type(ty1), Self::Type(ty2)) => {
                ty1.read().unwrap().deref() == ty2.read().unwrap().deref()
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deleted(arg0) => f.debug_tuple("Deleted").field(arg0).finish(),
            Self::JSON(arg0) => f
                .debug_tuple("JSON")
                .field(&format!("Vec [len: {}]", arg0.len()))
                .finish(),
            Self::Binary(arg0) => f
                .debug_tuple("Binary")
                .field(&format!("Binary [len: {}]", arg0.len()))
                .finish(),
            Self::String(arg0) => f.debug_tuple("String").field(arg0).finish(),
            Self::Embed(arg0) => f.debug_tuple("Embed").field(arg0).finish(),
            Self::Format { key, value } => f
                .debug_struct("Format")
                .field("key", key)
                .field("value", value)
                .finish(),
            Self::Type(arg0) => f.debug_tuple("Type").field(arg0).finish(),
            Self::Any(arg0) => f.debug_tuple("Any").field(arg0).finish(),
            Self::Doc { guid, opts } => f
                .debug_struct("Doc")
                .field("guid", guid)
                .field("opts", opts)
                .finish(),
        }
    }
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
                let kind = YTypeKind::from(type_ref);
                let tag_name = match kind {
                    YTypeKind::XMLElement | YTypeKind::XMLHook => Some(decoder.read_var_string()?),
                    YTypeKind::Unknown => {
                        return Err(JwstCodecError::IncompleteDocument(format!(
                            "Unknown y type: {type_ref}"
                        )));
                    }
                    _ => None,
                };

                let ty = YType::new(kind, tag_name);
                Ok(Self::Type(ty.into_ref()))
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
            Self::Type(ty) => {
                let ty = ty.read().unwrap();
                let type_ref = u64::from(ty.kind());
                encoder.write_var_u64(type_ref)?;

                match ty.kind {
                    YTypeKind::XMLElement | YTypeKind::XMLHook => {
                        encoder.write_var_string(ty.name.as_ref().unwrap())?;
                    }
                    _ => {}
                }
            }
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
            Self::String(string) => string.chars().count() as u64,
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

    pub fn at(&self, index: u64) -> JwstCodecResult<Option<Self>> {
        match self {
            Self::Deleted(_) => Ok(None),
            Self::JSON(strings) => {
                if index < strings.len() as u64 {
                    if let Some(string) = &strings[index as usize] {
                        return Ok(Some(Self::String(string.clone())));
                    }
                }
                Ok(None)
            }
            Self::String(string) => {
                if index < string.len() as u64 {
                    Ok(Some(Self::String(
                        string[index as usize..=index as usize].to_string(),
                    )))
                } else {
                    Ok(None)
                }
            }
            Self::Any(any) => {
                if index < any.len() as u64 {
                    Ok(Some(Self::Any(vec![any[index as usize].clone()])))
                } else {
                    Ok(None)
                }
            }
            Self::Binary(_)
            | Self::Embed(_)
            | Self::Format { .. }
            | Self::Type(_)
            | Self::Doc { .. } => {
                if index == 0 {
                    Ok(Some(self.clone()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn splittable(&self) -> bool {
        matches!(
            self,
            Self::String { .. } | Self::Any { .. } | Self::JSON { .. }
        )
    }

    pub fn split(&self, diff: u64) -> JwstCodecResult<(Self, Self)> {
        // TODO: implement split for other types
        match self {
            Self::String(str) => {
                let (left, right) = Self::split_as_utf16_str(str.as_str(), diff);
                Ok((
                    Self::String(left.to_string()),
                    Self::String(right.to_string()),
                ))
            }
            Self::JSON(vec) => {
                let (left, right) = vec.split_at((diff + 1) as usize);
                Ok((Self::JSON(left.to_owned()), Self::JSON(right.to_owned())))
            }
            Self::Any(vec) => {
                let (left, right) = vec.split_at((diff + 1) as usize);
                Ok((Self::Any(left.to_owned()), Self::Any(right.to_owned())))
            }
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
            Content::Type(YType::new(YTypeKind::Array, None).into_ref()),
            Content::Type(YType::new(YTypeKind::Map, None).into_ref()),
            Content::Type(YType::new(YTypeKind::Text, None).into_ref()),
            Content::Type(YType::new(YTypeKind::XMLElement, Some("test".to_string())).into_ref()),
            Content::Type(YType::new(YTypeKind::XMLFragment, None).into_ref()),
            Content::Type(YType::new(YTypeKind::XMLHook, Some("test".to_string())).into_ref()),
            Content::Type(YType::new(YTypeKind::XMLText, None).into_ref()),
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
