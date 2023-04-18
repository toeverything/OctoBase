mod codec;
mod doc;

pub use codec::{
    read_var_buffer, read_var_i64, read_var_string, read_var_u64, write_var_i64, write_var_u64,
};
pub use doc::{read_content, read_item, read_item_id, read_update, Content, Id, Item, Update};

use nom::IResult;

pub fn parse_doc_update(input: &[u8]) -> IResult<&[u8], Update> {
    let (input, update) = read_update(input)?;
    // debug_assert_eq!(input.len(), 0);
    Ok((input, update))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_doc() {
        let (_tail, update) = parse_doc_update(include_bytes!("./fixtures/basic_doc.bin")).unwrap();

        assert_eq!(update.structs[0].structs.len(), 188);
    }
}
