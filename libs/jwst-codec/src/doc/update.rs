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
    Item(Item),
}

#[derive(Debug)]
pub struct Delete {
    pub clock: u64,
    pub clock_len: u64,
}

#[derive(Debug)]
pub struct DeleteSets {
    pub client: u64,
    pub deletes: Vec<Delete>,
}

#[derive(Debug)]
pub struct Update {
    pub delete_sets: Vec<DeleteSets>,
    pub structs: HashMap<u64, Vec<StructInfo>>,
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
                vec.push(StructInfo::Item(item));
                (vec, clock + len)
            }
        });

    Ok((input, RawRefs { client, refs }))
}

fn read_client_struct_refs(input: &[u8]) -> IResult<&[u8], HashMap<u64, Vec<StructInfo>>> {
    let (input, num_of_updates) = read_var_u64(input)?;
    let (tail, updates) = count(read_refs, num_of_updates as usize)(input)?;

    Ok((
        tail,
        updates.into_iter().map(|u| (u.client, u.refs)).collect(),
    ))
}

fn read_delete(input: &[u8]) -> IResult<&[u8], Delete> {
    let (tail, clock) = read_var_u64(input)?;
    let (tail, clock_len) = read_var_u64(tail)?;
    Ok((tail, Delete { clock, clock_len }))
}

fn parse_delete_set(input: &[u8]) -> IResult<&[u8], DeleteSets> {
    let (input, client) = read_var_u64(input)?;
    let (input, num_of_deletes) = read_var_u64(input)?;
    let (tail, deletes) = count(read_delete, num_of_deletes as usize)(input)?;

    Ok((tail, DeleteSets { client, deletes }))
}

fn read_delete_set(input: &[u8]) -> IResult<&[u8], Vec<DeleteSets>> {
    let (input, num_of_clients) = read_var_u64(input)?;
    let (tail, deletes) = count(parse_delete_set, num_of_clients as usize)(input)?;

    Ok((tail, deletes))
}

pub fn read_update(input: &[u8]) -> IResult<&[u8], Update> {
    let (tail, structs) = read_client_struct_refs(input)?;
    let (tail, delete_sets) = read_delete_set(tail)?;
    Ok((
        tail,
        Update {
            structs,
            delete_sets,
        },
    ))
}
