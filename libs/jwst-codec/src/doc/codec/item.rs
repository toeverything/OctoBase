use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Parent {
    String(String),
    Id(Id),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub left_id: Option<Id>,
    pub right_id: Option<Id>,
    pub parent: Option<Parent>,
    pub parent_sub: Option<String>,
    pub content: Content,
}

fn read_has_parent(input: &[u8]) -> IResult<&[u8], bool> {
    let (tail, info) = read_var_u64(input)?;
    let has_parent = info == 1;
    Ok((tail, has_parent))
}

pub fn read_item(input: &[u8], info: u8, first_5_bit: u8) -> IResult<&[u8], Item> {
    let mut input = input;
    let has_left_id = info & 0b1000_0000 == 0b1000_0000;
    let has_right_id = info & 0b0100_0000 == 0b0100_0000;
    let has_parent_sub = info & 0b0010_0000 == 0b0010_0000;
    let has_not_parent_info = info & 0b1100_0000 == 0;

    // NOTE: read order must keep the same as the order in yjs
    // TODO: this data structure design will break the cpu OOE, need to be optimized
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
            if has_not_parent_info {
                let (tail, has_parent) = read_has_parent(input)?;
                input = tail;
                Some(if has_parent {
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
        parent_sub: if has_not_parent_info && has_parent_sub {
            let (tail, parent_sub) = read_var_string(input)?;
            input = tail;
            Some(parent_sub)
        } else {
            None
        },
        content: {
            // tag must not GC or Skip, this must process in parse_struct
            debug_assert_ne!(first_5_bit, 0);
            debug_assert_ne!(first_5_bit, 10);
            let (tail, content) = read_content(input, first_5_bit)?;
            input = tail;
            content
        },
    };

    Ok((input, item))
}
