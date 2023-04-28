mod doc;
mod sync;

pub use sync::{read_sync_message, write_sync_message, SyncMessage};

use super::*;
use doc::{read_doc_message, write_doc_message, DocMessage};
use jwst_codec::{
    read_var_buffer, read_var_string, read_var_u64, write_var_buffer, write_var_string,
    write_var_u64,
};
use nom::{
    error::{Error, ErrorKind},
    IResult,
};
use std::collections::HashMap;
use std::io::{Error as IoError, Write};

struct AwarenessClientState {
    clock: u32,
    // content is usually a json
    content: String,
}

// awareness state message
pub struct AwarenessMessage {
    clients: HashMap<u64, AwarenessClientState>,
}
