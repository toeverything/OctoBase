#[forbid(unsafe_code)]
mod codec;
mod doc;
mod protocol;
mod sync;

pub use codec::*;
pub use doc::{
    decode_maybe_update_with_guid, decode_update_with_guid, encode_update_as_message, encode_update_with_guid,
    merge_updates_v1, Any, Array, Awareness, AwarenessEvent, Client, Clock, CrdtRead, CrdtReader, CrdtWrite,
    CrdtWriter, Doc, DocOptions, Id, Map, RawDecoder, RawEncoder, StateVector, Text, Update, Value,
};
pub(crate) use doc::{Content, Item, HASHMAP_SAFE_CAPACITY};
pub use jwst_logger::{debug, error, info, trace, warn};
use nom::IResult;
pub use protocol::{
    read_sync_message, write_sync_message, AwarenessState, AwarenessStates, DocMessage, SyncMessage, SyncMessageScanner,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum JwstCodecError {
    #[error("Unexpected Scenario")]
    Unexpected,
    #[error("Damaged document: corrupt json data")]
    DamagedDocumentJson,
    #[error("Incomplete document: {0}")]
    IncompleteDocument(String),
    #[error("Invalid write buffer: {0}")]
    InvalidWriteBuffer(String),
    #[error("Content does not support splitting in {0}")]
    ContentSplitNotSupport(u64),
    #[error("GC or Skip does not support splitting")]
    ItemSplitNotSupport,
    #[error("update is empty")]
    UpdateIsEmpty,
    #[error("invalid update")]
    UpdateInvalid(#[from] nom::Err<nom::error::Error<usize>>),
    #[error("update not fully consumed: {0}")]
    UpdateNotFullyConsumed(usize),
    #[error("invalid struct clock, expect {expect}, actually {actually}")]
    StructClockInvalid { expect: u64, actually: u64 },
    #[error("cannot find struct {clock} in {client_id}")]
    StructSequenceInvalid { client_id: u64, clock: u64 },
    #[error("struct {0} not exists")]
    StructSequenceNotExists(u64),
    #[error("Invalid parent")]
    InvalidParent,
    #[error("Parent not found")]
    ParentNotFound,
    #[error("Invalid struct type, expect item, actually {0}")]
    InvalidStructType(&'static str),
    #[error("Can not cast known type to {0}")]
    TypeCastError(&'static str),
    #[error("Can not found root struct with name: {0}")]
    RootStructNotFound(String),
    #[error("Index {0} out of bound")]
    IndexOutOfBound(u64),
    #[error("Document has been released")]
    DocReleased,
    #[error("Unexpected type, expect {0}")]
    UnexpectedType(&'static str),
}

pub type JwstCodecResult<T = ()> = Result<T, JwstCodecError>;
