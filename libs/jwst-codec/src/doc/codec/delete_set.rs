use crate::doc::OrderRange;

use super::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    ops::{Deref, DerefMut, Range},
};

impl<R: CrdtReader> CrdtRead<R> for Range<u64> {
    fn read(decoder: &mut R) -> JwstCodecResult<Self> {
        let clock = decoder.read_var_u64()?;
        let len = decoder.read_var_u64()?;
        Ok(clock..clock + len)
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for Range<u64> {
    fn write(&self, encoder: &mut W) -> JwstCodecResult {
        encoder.write_var_u64(self.start)?;
        encoder.write_var_u64(self.end - self.start)?;
        Ok(())
    }
}

impl<R: CrdtReader> CrdtRead<R> for OrderRange {
    fn read(decoder: &mut R) -> JwstCodecResult<Self> {
        let num_of_deletes = decoder.read_var_u64()? as usize;
        if num_of_deletes == 1 {
            Ok(OrderRange::Range(Range::<u64>::read(decoder)?))
        } else {
            let mut deletes = Vec::with_capacity(num_of_deletes);

            for _ in 0..num_of_deletes {
                deletes.push(Range::<u64>::read(decoder)?);
            }

            Ok(OrderRange::Fragment(deletes))
        }
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for OrderRange {
    fn write(&self, encoder: &mut W) -> JwstCodecResult {
        match self {
            OrderRange::Range(range) => {
                encoder.write_var_u64(1)?;
                range.write(encoder)?;
            }
            OrderRange::Fragment(ranges) => {
                encoder.write_var_u64(ranges.len() as u64)?;
                for range in ranges {
                    range.write(encoder)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DeleteSet(pub HashMap<Client, OrderRange>);

impl Deref for DeleteSet {
    type Target = HashMap<Client, OrderRange>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> From<[(Client, Vec<Range<u64>>); N]> for DeleteSet {
    fn from(value: [(Client, Vec<Range<u64>>); N]) -> Self {
        let mut map = HashMap::with_capacity(N);
        for (client, ranges) in value {
            map.insert(client, ranges.into());
        }
        Self(map)
    }
}

impl DerefMut for DeleteSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DeleteSet {
    pub fn add(&mut self, client: Client, from: Clock, len: Clock) {
        self.add_range(client, from..from + len);
    }

    pub fn add_range(&mut self, client: Client, range: Range<u64>) {
        match self.0.entry(client) {
            Entry::Occupied(e) => {
                let r = e.into_mut();
                if r.is_empty() {
                    *r = range.into();
                } else {
                    r.push(range);
                }
            }
            Entry::Vacant(e) => {
                e.insert(range.into());
            }
        }
    }

    pub fn batch_push(&mut self, client: Client, ranges: Vec<Range<u64>>) {
        match self.0.entry(client) {
            Entry::Occupied(e) => {
                e.into_mut().extends(ranges);
            }
            Entry::Vacant(e) => {
                e.insert(ranges.into());
            }
        }
    }

    pub fn merge(&mut self, other: Self) {
        for (client, range) in other.0 {
            match self.0.entry(client) {
                Entry::Occupied(e) => {
                    e.into_mut().merge(range);
                }
                Entry::Vacant(e) => {
                    e.insert(range);
                }
            }
        }
    }
}

impl<R: CrdtReader> CrdtRead<R> for DeleteSet {
    fn read(decoder: &mut R) -> JwstCodecResult<Self> {
        let num_of_clients = decoder.read_var_u64()? as usize;
        let mut map = HashMap::with_capacity(num_of_clients);

        for _ in 0..num_of_clients {
            let client = decoder.read_var_u64()?;
            let deletes = OrderRange::read(decoder)?;
            map.insert(client, deletes);
        }

        Ok(DeleteSet(map))
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for DeleteSet {
    fn write(&self, encoder: &mut W) -> JwstCodecResult {
        encoder.write_var_u64(self.len() as u64)?;
        for (client, deletes) in self.iter() {
            encoder.write_var_u64(*client)?;
            deletes.write(encoder)?;
        }

        Ok(())
    }
}
