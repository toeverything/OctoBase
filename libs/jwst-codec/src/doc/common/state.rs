use crate::{Client, Clock, Id};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Default, Debug, PartialEq, Clone)]
pub struct StateVector(HashMap<Client, Clock>);

impl StateVector {
    pub fn set_max(&mut self, client: Client, clock: Clock) {
        self.entry(client)
            .and_modify(|m_clock| {
                if *m_clock < clock {
                    *m_clock = clock;
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
            .and_modify(|m_clock| {
                if *m_clock > clock {
                    *m_clock = clock;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_vector_basic() {
        let mut state_vector = StateVector::from([(1, 1), (2, 2), (3, 3)]);
        assert_eq!(state_vector.len(), 3);
        assert_eq!(state_vector.get(&1), 1);

        state_vector.set_min(1, 0);
        assert_eq!(state_vector.get(&1), 0);

        state_vector.set_max(1, 4);
        assert_eq!(state_vector.get(&1), 4);

        // set inexistent client
        state_vector.set_max(4, 1);
        assert_eq!(state_vector.get(&4), 1);

        // same client with larger clock
        assert!(!state_vector.contains(&(1, 5).into()));
    }

    #[test]
    fn test_state_vector_merge() {
        let mut state_vector = StateVector::from([(1, 1), (2, 2), (3, 3)]);
        let other_state_vector = StateVector::from([(1, 5), (2, 6), (3, 7)]);
        state_vector.merge_with(&other_state_vector);
        assert_eq!(state_vector, StateVector::from([(3, 3), (1, 1), (2, 2)]));
    }
}
