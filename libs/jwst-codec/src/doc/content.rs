use super::*;

#[derive(Debug)]
pub enum Content {
    GC,
    Deleted,
    JSON,
    Binary,
    String,
    Embed,
    Format,
    Type,
    Any,
    Doc,
    Skip,
}

pub fn read_content(input: &[u8]) -> IResult<&[u8], Content> {
    let tail = input;
    Ok((tail, Content::GC))
}
