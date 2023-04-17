use super::*;
use nom::{bytes::complete::take, multi::count};

#[derive(Debug)]
pub enum StructInfo {
    GC,
    Skip,
    Item(Item),
}

#[derive(Debug)]
pub struct Structs {
    client: u64,
    clock: u64,
    structs: Vec<StructInfo>,
}

#[derive(Debug)]
pub struct Update {
    structs: Vec<Structs>,
}

fn parse_struct(input: &[u8]) -> IResult<&[u8], StructInfo> {
    let (input, info) = take(1u8)(input)?;
    let info = read_var_u64(info)?.1;
    let first_5_bits = info & 0b11111;

    match first_5_bits {
        0 => Ok((input, StructInfo::GC)),
        10 => Ok((input, StructInfo::Skip)),
        _ => {
            let (input, item) = read_item(input, info)?;
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

pub fn read_client_struct_refs(input: &[u8]) -> IResult<&[u8], Vec<Structs>> {
    let (input, num_of_updates) = read_var_u64(input)?;
    let (tail, updates) = count(parse_structs, num_of_updates as usize)(input)?;

    Ok((tail, updates))
}

pub fn read_update(input: &[u8]) -> IResult<&[u8], Update> {
    let (input, structs) = read_client_struct_refs(input)?;
    Ok((input, Update { structs }))
}
