use super::*;
use nom::multi::count;
use std::collections::HashMap;

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
