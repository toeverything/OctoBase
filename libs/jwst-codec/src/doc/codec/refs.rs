use super::*;
use nom::{multi::count, number::complete::be_u8};
use std::collections::HashMap;

enum RawStructInfo {
    GC(u64),
    Skip(u64),
    Item(Item),
}

struct RawRefs {
    client: u64,
    refs: Vec<StructInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructInfo {
    GC { id: Id, len: u64 },
    Skip { id: Id, len: u64 },
    Item { id: Id, item: Item },
}

impl StructInfo {
    pub fn id(&self) -> &Id {
        match self {
            StructInfo::GC { id, .. } => id,
            StructInfo::Skip { id, .. } => id,
            StructInfo::Item { id, .. } => id,
        }
    }

    pub fn client_id(&self) -> u64 {
        self.id().client
    }

    pub fn clock(&self) -> u64 {
        self.id().clock
    }

    pub fn len(&self) -> u64 {
        match self {
            StructInfo::GC { len, .. } => *len,
            StructInfo::Skip { len, .. } => *len,
            StructInfo::Item { item, .. } => item.content.clock_len(),
        }
    }

    pub fn is_gc(&self) -> bool {
        matches!(self, StructInfo::GC { .. })
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, StructInfo::Skip { .. })
    }

    pub fn is_item(&self) -> bool {
        matches!(self, StructInfo::Item { .. })
    }

    pub fn split_item(&mut self, diff: u64) -> JwstCodecResult<(Self, Self)> {
        if let Self::Item { id, item } = self {
            let right_id = Id::new(id.client, id.clock + diff);
            let (left_content, right_content) = item.content.split(diff)?;

            let left_item = StructInfo::Item {
                id: id.clone(),
                item: Item {
                    right_id: Some(right_id.clone()),
                    content: left_content,
                    ..item.clone()
                },
            };

            let right_item = StructInfo::Item {
                id: right_id,
                item: Item {
                    left_id: Some(Id::new(id.client, id.clock + diff - 1)),
                    right_id: item.right_id.clone(),
                    parent: item.parent.clone(),
                    parent_sub: item.parent_sub.clone(),
                    content: right_content,
                },
            };

            Ok((left_item, right_item))
        } else {
            Err(JwstCodecError::ItemSplitNotSupport)
        }
    }
}

fn read_struct(input: &[u8]) -> IResult<&[u8], RawStructInfo> {
    let (input, info) = be_u8(input)?;
    let first_5_bit = info & 0b11111;

    match first_5_bit {
        0 => {
            let (input, len) = read_var_u64(input)?;
            Ok((input, RawStructInfo::GC(len)))
        }
        10 => {
            let (input, len) = read_var_u64(input)?;
            Ok((input, RawStructInfo::Skip(len)))
        }
        _ => {
            let (input, item) = read_item(input, info, first_5_bit)?;
            Ok((input, RawStructInfo::Item(item)))
        }
    }
}

fn read_refs(input: &[u8]) -> IResult<&[u8], RawRefs> {
    let (input, num_of_structs) = read_var_u64(input)?;
    let (input, client) = read_var_u64(input)?;
    let (input, clock) = read_var_u64(input)?;
    let (input, structs) = count(read_struct, num_of_structs as usize)(input)?;
    let (refs, _) = structs
        .into_iter()
        .fold((vec![], clock), |(mut vec, clock), s| {
            let id = Id::new(client, clock);
            match s {
                RawStructInfo::GC(len) => {
                    vec.push(StructInfo::GC { id, len });
                    (vec, clock + len)
                }
                RawStructInfo::Skip(len) => {
                    vec.push(StructInfo::Skip { id, len });
                    (vec, clock + len)
                }
                RawStructInfo::Item(item) => {
                    let len = item.content.clock_len();
                    vec.push(StructInfo::Item { id, item });
                    (vec, clock + len)
                }
            }
        });

    Ok((input, RawRefs { client, refs }))
}

pub fn read_client_struct_refs(input: &[u8]) -> IResult<&[u8], HashMap<u64, Vec<StructInfo>>> {
    let (input, num_of_updates) = read_var_u64(input)?;
    let (tail, updates) = count(read_refs, num_of_updates as usize)(input)?;

    Ok((
        tail,
        updates.into_iter().map(|u| (u.client, u.refs)).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_info() {
        {
            let struct_info = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            assert_eq!(struct_info.len(), 10);
            assert_eq!(struct_info.client_id(), 1);
            assert_eq!(struct_info.clock(), 0);
        }

        {
            let struct_info = StructInfo::Skip {
                id: Id::new(2, 0),
                len: 20,
            };
            assert_eq!(struct_info.len(), 20);
            assert_eq!(struct_info.client_id(), 2);
            assert_eq!(struct_info.clock(), 0);
        }
    }
}
