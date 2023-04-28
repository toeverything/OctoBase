use super::*;
use yrs::{StateVector, Update};

pub enum SyncMessage {
    Step1(StateVector),
    Step2(Update),
    Update(Update),
}
