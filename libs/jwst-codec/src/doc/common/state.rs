use crate::{Client, Clock, Id};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Default, Debug)]
pub struct StateVector(HashMap<Client, Clock>);

impl StateVector {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set_max(&mut self, client: Client, clock: Clock) {
        self.entry(client)
            .and_modify(|mclock| {
                if *mclock < clock {
                    *mclock = clock;
                }
            })
            .or_insert(clock);
    }

    pub fn get(&self, client: &Client) -> Clock {
        *self.0.get(client).unwrap_or(&0)
    }

    pub fn contains(&self, id: &Id) -> bool {
        id.clock <= self.get(&id.client)
    }

    pub fn set_min(&mut self, client: Client, clock: Clock) {
        self.entry(client)
            .and_modify(|mclock| {
                if *mclock > clock {
                    *mclock = clock;
                }
            })
            .or_insert(clock);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Client, &Clock)> {
        self.0.iter()
    }

    pub fn merge_with(&mut self, other: &Self) {
        for (client, clock) in other.iter() {
            self.set_min(*client, *clock);
        }
    }
}

impl Deref for StateVector {
    type Target = HashMap<Client, Clock>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StateVector {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> From<[(Client, Clock); N]> for StateVector {
    fn from(value: [(Client, Clock); N]) -> Self {
        let mut map = HashMap::with_capacity(N);

        for (client, clock) in value {
            map.insert(client, clock);
        }

        Self(map)
    }
}
