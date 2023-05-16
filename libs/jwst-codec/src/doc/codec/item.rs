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

pub fn read_item<R: CrdtReader>(
    decoder: &mut R,
    info: u8,
    first_5_bit: u8,
) -> JwstCodecResult<Item> {
    let has_left_id = info & 0b1000_0000 == 0b1000_0000;
    let has_right_id = info & 0b0100_0000 == 0b0100_0000;
    let has_parent_sub = info & 0b0010_0000 == 0b0010_0000;
    let has_not_parent_info = info & 0b1100_0000 == 0;

    // NOTE: read order must keep the same as the order in yjs
    // TODO: this data structure design will break the cpu OOE, need to be optimized
    let item = Item {
        left_id: if has_left_id {
            Some(decoder.read_item_id()?)
        } else {
            None
        },
        right_id: if has_right_id {
            Some(decoder.read_item_id()?)
        } else {
            None
        },
        parent: {
            if has_not_parent_info {
                let has_parent = decoder.read_var_u64()? == 1;
                Some(if has_parent {
                    Parent::String(decoder.read_var_string()?)
                } else {
                    Parent::Id(decoder.read_item_id()?)
                })
            } else {
                None
            }
        },
        parent_sub: if has_not_parent_info && has_parent_sub {
            Some(decoder.read_var_string()?)
        } else {
            None
        },
        content: {
            // tag must not GC or Skip, this must process in parse_struct
            debug_assert_ne!(first_5_bit, 0);
            debug_assert_ne!(first_5_bit, 10);
            decoder.read_content(first_5_bit)?
        },
    };

    Ok(item)
}
