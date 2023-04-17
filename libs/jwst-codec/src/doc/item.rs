use super::*;
use nom::IResult;

pub enum Parent {
    String(String),
    Id(Id),
}

pub struct Item {
    left_id: Option<Id>,
    right_id: Option<Id>,
    parent: Option<Parent>,
}

fn read_has_parent(input: &[u8]) -> IResult<&[u8], bool> {
    let (tail, info) = read_var_u64(input)?;
    let has_parent = info == 1;
    Ok((tail, has_parent))
}

pub fn read_item(input: &[u8], info: u64) -> IResult<&[u8], Item> {
    let mut input = input;
    let has_left_id = info & 0b1000_0000 == 0b1000_0000;
    let has_right_id = info & 0b0100_0000 == 0b1000_0000;
    let has_not_parent_info = !has_left_id && !has_right_id;

    let item = Item {
        left_id: if has_left_id {
            let (tail, id) = read_item_id(input)?;
            input = tail;
            Some(id)
        } else {
            None
        },
        right_id: if has_right_id {
            let (tail, id) = read_item_id(input)?;
            input = tail;
            Some(id)
        } else {
            None
        },
        parent: {
            let (tail, has_parent) = read_has_parent(input)?;
            input = tail;
            if has_parent {
                Some(if has_not_parent_info {
                    let (tail, parent) = read_var_string(input)?;
                    input = tail;
                    Parent::String(parent)
                } else {
                    let (tail, parent) = read_item_id(input)?;
                    input = tail;
                    Parent::Id(parent)
                })
            } else {
                None
            }
        },
    };

    Ok((input, item))
}
