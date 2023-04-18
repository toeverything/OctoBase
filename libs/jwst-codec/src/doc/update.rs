use super::*;
use nom::{multi::count, number::complete::be_u8};

#[derive(Debug)]
pub enum StructInfo {
    GC(u64),
    Skip(u64),
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
pub struct Structs {
    pub client: u64,
    pub clock: u64,
    pub structs: Vec<StructInfo>,
}

#[derive(Debug)]
pub struct Update {
    pub delete_sets: Vec<DeleteSets>,
    pub structs: Vec<Structs>,
}

fn parse_struct(input: &[u8]) -> IResult<&[u8], StructInfo> {
    let (input, info) = be_u8(input)?;
    let first_5_bit = info & 0b11111;

    match first_5_bit {
        0 => {
            let (input, len) = read_var_u64(input)?;
            Ok((input, StructInfo::GC(len)))
        }
        10 => {
            let (input, len) = read_var_u64(input)?;
            Ok((input, StructInfo::Skip(len)))
        }
        _ => {
            let (input, item) = read_item(input, info, first_5_bit)?;
            Ok((input, StructInfo::Item(item)))
        }
    }
}

fn parse_structs(input: &[u8]) -> IResult<&[u8], Structs> {
    let (input, num_of_structs) = read_var_u64(input)?;
    let (input, client) = read_var_u64(input)?;
    let (input, clock) = read_var_u64(input)?;
    let (input, structs) = count(parse_struct, num_of_structs as usize)(input)?;
    Ok((
        input,
        Structs {
            client,
            clock,
            structs,
        },
    ))
}

fn read_client_struct_refs(input: &[u8]) -> IResult<&[u8], Vec<Structs>> {
    let (input, num_of_updates) = read_var_u64(input)?;
    let (tail, updates) = count(parse_structs, num_of_updates as usize)(input)?;

    Ok((tail, updates))
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
