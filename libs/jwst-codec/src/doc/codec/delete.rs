use super::*;

#[derive(Debug)]
pub struct Delete {
    pub clock: Clock,
    pub len: u64,
}

impl Delete {
    fn from<R: CrdtReader>(decoder: &mut R) -> JwstCodecResult<Self> {
        let clock = decoder.read_var_u64()?;
        let len = decoder.read_var_u64()?;
        Ok(Delete { clock, len })
    }
}

#[derive(Debug)]
pub struct DeleteSets {
    pub client: u64,
    pub deletes: Vec<Delete>,
}

impl DeleteSets {
    pub(crate) fn from<R: CrdtReader>(decoder: &mut R) -> JwstCodecResult<Self> {
        let client = decoder.read_var_u64()?;
        let num_of_deletes = decoder.read_var_u64()?;
        let deletes = (0..num_of_deletes)
            .map(|_| Delete::from(decoder))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(DeleteSets { client, deletes })
    }
}
