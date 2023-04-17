use super::*;
use nom::{
    combinator::map,
    error::{Error, ErrorKind},
    multi::count,
};
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub enum Content {
    Deleted(u64),
    JSON(Vec<Option<String>>),
    Binary(Vec<u8>),
    String(String),
    Embed(JsonValue),
    Format,
    Type,
    Any,
    Doc,
}

pub fn read_content(input: &[u8], tag_type: u64) -> IResult<&[u8], Content> {
    match tag_type {
        1 => {
            let (tail, len) = read_var_u64(input)?;
            Ok((tail, Content::Deleted(len)))
        }
        2 => {
            let (tail, len) = read_var_u64(input)?;
            let (tail, strings) = count(
                map(read_var_string, |string| {
                    if string == "undefined" {
                        None
                    } else {
                        Some(string)
                    }
                }),
                len as usize,
            )(tail)?;
            Ok((tail, Content::JSON(strings)))
        }
        3 => {
            let (tail, bytes) = read_var_buffer(input)?;
            Ok((tail, Content::Binary(bytes.to_vec())))
        }
        4 => {
            let (tail, string) = read_var_string(input)?;
            Ok((tail, Content::String(string)))
        }
        _ => Err(nom::Err::Error(Error::new(input, ErrorKind::Tag))),
    }
}
