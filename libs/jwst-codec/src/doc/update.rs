use super::*;
use nom::{multi::count, number::complete::be_u8};

#[derive(Debug)]
pub enum StructInfo {
    GC,
    Skip,
    Item(Item),
}

#[derive(Debug)]
pub struct Structs {
    pub client: u64,
    pub clock: u64,
    pub structs: Vec<StructInfo>,
}

#[derive(Debug)]
pub struct Update {
    pub structs: Vec<Structs>,
}

fn parse_struct(input: &[u8]) -> IResult<&[u8], StructInfo> {
    let (input, info) = be_u8(input)?;
    let first_5_bit = info & 0b11111;

    match first_5_bit {
        0 => Ok((input, StructInfo::GC)),
        10 => Ok((input, StructInfo::Skip)),
        _ => {
            let (input, item) = read_item(input, info, first_5_bit).unwrap();
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
