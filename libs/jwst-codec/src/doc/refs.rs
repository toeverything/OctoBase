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

#[derive(Debug)]
pub enum StructInfo {
    GC { client: u64, clock: u64, len: u64 },
    Skip { client: u64, clock: u64, len: u64 },
    Item { client: u64, clock: u64, item: Item },
}

impl StructInfo {
    pub fn client_id(&self) -> u64 {
        match self {
            StructInfo::GC { client, .. } => *client,
            StructInfo::Skip { client, .. } => *client,
            StructInfo::Item { client, .. } => *client,
        }
    }

    pub fn clock(&self) -> u64 {
        match self {
            StructInfo::GC { clock, .. } => *clock,
            StructInfo::Skip { clock, .. } => *clock,
            StructInfo::Item { clock, .. } => *clock,
        }
    }

    pub fn len(&self) -> u64 {
        match self {
            StructInfo::GC { len, .. } => *len,
            StructInfo::Skip { len, .. } => *len,
            StructInfo::Item { item, .. } => item.content.clock_len(),
        }
    }

    pub fn is_gc(&self) -> bool {
        match self {
            StructInfo::GC { .. } => true,
            _ => false,
        }
    }

    pub fn is_skip(&self) -> bool {
        match self {
            StructInfo::Skip { .. } => true,
            _ => false,
        }
    }

    pub fn is_item(&self) -> bool {
        match self {
            StructInfo::Item { .. } => true,
            _ => false,
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
        .fold((vec![], clock), |(mut vec, clock), s| match s {
            RawStructInfo::GC(len) => {
                vec.push(StructInfo::GC { client, clock, len });
                (vec, clock + len)
            }
            RawStructInfo::Skip(len) => {
                vec.push(StructInfo::Skip { client, clock, len });
                (vec, clock + len)
            }
            RawStructInfo::Item(item) => {
                let len = item.content.clock_len();
                vec.push(StructInfo::Item {
                    client,
                    clock,
                    item,
                });
                (vec, clock + len)
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
