use jwst::{Block, Workspace};
use std::{ffi::CStr, os::raw::c_char};
use yrs::Transaction;

#[no_mangle]
pub unsafe extern "C" fn block_new(
    workspace: *const Workspace,
    trx: *mut Transaction,
    block_id: *const c_char,
    flavor: *const c_char,
    operator: u64,
) -> *mut Block {
    Box::into_raw(Box::new(Block::new(
        workspace.as_ref().unwrap(),
        trx.as_mut().unwrap(),
        CStr::from_ptr(block_id).to_str().unwrap(),
        CStr::from_ptr(flavor).to_str().unwrap(),
        operator,
    )))
}

#[no_mangle]
pub unsafe extern "C" fn workspace_new(trx: *mut Transaction, id: *const c_char) -> *mut Workspace {
    Box::into_raw(Box::new(Workspace::new(
        trx.as_mut().unwrap(),
        CStr::from_ptr(id).to_str().unwrap(),
    )))
}
