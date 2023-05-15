use std::{
    cmp::Ordering,
    hash::Hash,
    ops::{Add, Sub},
};

pub type Client = u64;
pub type Clock = u64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Id {
    pub client: Client,
    pub clock: Clock,
}

impl Id {
    pub fn new(client: Client, clock: Clock) -> Self {
        Self { client, clock }
    }
}

impl From<(Client, Clock)> for Id {
    fn from((client, clock): (Client, Clock)) -> Self {
        Id::new(client, clock)
    }
}

impl Sub<Clock> for Id {
    type Output = Id;

    fn sub(self, rhs: Clock) -> Self::Output {
        (self.client, self.clock - rhs).into()
    }
}

impl Add<Clock> for Id {
    type Output = Id;

    fn add(self, rhs: Clock) -> Self::Output {
        (self.client, self.clock + rhs).into()
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.client.cmp(&other.client) {
            Ordering::Equal => Some(self.clock.cmp(&other.clock)),
            _ => None,
        }
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.clock.cmp(&other.clock)
    }
}
