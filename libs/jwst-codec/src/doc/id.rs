use nom::IResult;

use super::*;

pub struct Id {
    client: u64,
    clock: u64,
}

pub fn read_item_id(input: &[u8]) -> IResult<&[u8], Id> {
    let (tail, client) = read_var_u64(input)?;
    let (tail, clock) = read_var_u64(tail)?;
    Ok((tail, Id { client, clock }))
}
