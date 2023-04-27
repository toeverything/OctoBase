use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Id {
    pub client: u64,
    pub clock: u64,
}

impl Id {
    pub fn new(client: u64, clock: u64) -> Self {
        Self { client, clock }
    }
}

pub fn read_item_id(input: &[u8]) -> IResult<&[u8], Id> {
    let (tail, client) = read_var_u64(input)?;
    let (tail, clock) = read_var_u64(tail)?;
    Ok((tail, Id::new(client, clock)))
}
