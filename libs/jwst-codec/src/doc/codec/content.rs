use super::*;
use nom::{
    combinator::map,
    error::{Error, ErrorKind},
    multi::count,
};
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
    pub fn clock_len(&self) -> u64 {
        match self {
            Content::Deleted(len) => *len,
            Content::JSON(strings) => strings.len() as u64,
            Content::String(string) => string.len() as u64,
            Content::Any(any) => any.len() as u64,
            Content::Binary(_)
            | Content::Embed(_)
            | Content::Format { .. }
            | Content::Type(_)
            | Content::Doc { .. } => 1,
        }
    }

    pub fn split(&self, diff: u64) -> JwstCodecResult<(Content, Content)> {
        // TODO: implement split for other types
        Err(JwstCodecError::ContentSplitNotSupport(diff))
    }
}

pub fn read_content(input: &[u8], tag_type: u8) -> IResult<&[u8], Content> {
    match tag_type {
        1 => {
            let (tail, len) = read_var_u64(input)?;
            Ok((tail, Content::Deleted(len)))
        } // Deleted
        2 => {
            let (tail, len) = read_var_u64(input)?;
            let (tail, strings) = count(
                map(read_var_string, |string| {
                    (string != "undefined").then_some(string)
                }),
                len as usize,
            )(tail)?;
            Ok((tail, Content::JSON(strings)))
        } // JSON
        3 => {
            let (tail, bytes) = read_var_buffer(input)?;
            Ok((tail, Content::Binary(bytes.to_vec())))
        } // Binary
        4 => {
            let (tail, string) = read_var_string(input)?;
            Ok((tail, Content::String(string)))
        } // String
        5 => {
            let (tail, string) = read_var_string(input)?;

            let json = serde_json::from_str(&string).map_err(|_| {
                nom::Err::Error(Error::new(&input[1..string.len() + 1], ErrorKind::Verify))
            })?;
            Ok((tail, Content::Embed(json)))
        } // Embed
        6 => {
            let (tail, key) = read_var_string(input)?;
            let (tail, value) = read_var_string(tail)?;
            let value = serde_json::from_str(&value).map_err(|_| {
                nom::Err::Error(Error::new(&input[1..value.len() + 1], ErrorKind::Verify))
            })?;
            Ok((tail, Content::Format { key, value }))
        } // Format
        7 => {
            let (tail, type_ref) = read_var_u64(input)?;
            let (tail, ytype) = match type_ref {
                0 => (tail, YType::Array),
                1 => (tail, YType::Map),
                2 => (tail, YType::Text),
                3 => {
                    let (tail, name) = read_var_string(tail)?;
                    (tail, YType::XmlElement(name))
                }
                4 => (tail, YType::XmlFragment),
                5 => {
                    let (tail, name) = read_var_string(tail)?;
                    (tail, YType::XmlHook(name))
                }
                6 => (tail, YType::XmlText),
                _ => return Err(nom::Err::Error(Error::new(input, ErrorKind::Tag))),
            };
            Ok((tail, Content::Type(ytype)))
        } // YType
        8 => {
            let (tail, any) = read_any(input)?;
            Ok((tail, Content::Any(any)))
        } // Any
        9 => {
            let (tail, guid) = read_var_string(input)?;
            let (tail, opts) = read_any(tail)?;
            Ok((tail, Content::Doc { guid, opts }))
        } // Doc
        _ => Err(nom::Err::Error(Error::new(input, ErrorKind::Tag))),
    }
}
