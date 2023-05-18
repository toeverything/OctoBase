use std::{
    hash::Hash,
    ops::{Add, Sub},
};

pub type Client = u64;
pub type Clock = u64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
