mod sync;

pub use sync::SyncMessage;

use super::*;
use jwst_codec::{read_var_buffer, read_var_i64, read_var_string, read_var_u64};
use nom::{
    error::{Error, ErrorKind},
    IResult,
};
use std::collections::HashMap;

struct AwarenessClientState {
    clock: u32,
    // content is usually a json
    content: String,
}

// awareness state message
pub struct AwarenessMessage {
    clients: HashMap<u64, AwarenessClientState>,
}
