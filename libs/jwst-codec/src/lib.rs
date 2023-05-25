#[forbid(unsafe_code)]
mod codec;
mod doc;
mod protocol;

pub use codec::{
    read_var_buffer, read_var_i64, read_var_string, read_var_u64, write_var_buffer, write_var_i64,
    write_var_string, write_var_u64,
};
pub use doc::{
    Any, Awareness, AwarenessEvent, Client, Clock, Content, CrdtRead, CrdtReader, CrdtWrite,
    CrdtWriter, Doc, Id, Item, RawDecoder, RawEncoder, Update,
};
pub use protocol::{
    read_sync_message, write_sync_message, AwarenessState, AwarenessStates, DocMessage,
    SyncMessage, SyncMessageScanner,
};

use jwst_logger::warn;
use nanoid::nanoid;
use nom::IResult;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum JwstCodecError {
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
}

pub type JwstCodecResult<T = ()> = Result<T, JwstCodecError>;
