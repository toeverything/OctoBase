use jwst::{Block, Workspace};
use std::{ffi::CStr, os::raw::c_char, ptr};
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
pub unsafe extern "C" fn block_destroy(block: *mut Block) {
    drop(Box::from_raw(block));
}

#[no_mangle]
pub unsafe extern "C" fn block_get_created(block: *const Block) -> u64 {
    block.as_ref().unwrap().created()
}

#[no_mangle]
pub unsafe extern "C" fn block_get_updated(block: *const Block) -> u64 {
    block.as_ref().unwrap().updated()
}

#[no_mangle]
pub unsafe extern "C" fn workspace_new(trx: *mut Transaction, id: *const c_char) -> *mut Workspace {
    Box::into_raw(Box::new(Workspace::new(
        trx.as_mut().unwrap(),
        CStr::from_ptr(id).to_str().unwrap(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn workspace_destroy(workspace: *mut Workspace) {
    drop(Box::from_raw(workspace));
}

#[no_mangle]
pub unsafe extern "C" fn workspace_get_block(
    workspace: *const Workspace,
    block_id: *const c_char,
    operator: u64,
) -> *mut Block {
    let block = workspace
        .as_ref()
        .unwrap()
        .get(CStr::from_ptr(block_id).to_str().unwrap(), operator);

    if let Some(block) = block {
        Box::leak(Box::new(block))
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn workspace_create_block(
    workspace: *const Workspace,
    trx: *mut Transaction,
    block_id: *const c_char,
    flavor: *const c_char,
    operator: u64,
) -> *mut Block {
    let block_id = CStr::from_ptr(block_id).to_str().unwrap();
    let flavor = CStr::from_ptr(flavor).to_str().unwrap();
    let block =
        workspace
            .as_ref()
            .unwrap()
            .create(trx.as_mut().unwrap(), block_id, flavor, operator);

    Box::into_raw(Box::new(block))
}

#[no_mangle]
pub unsafe extern "C" fn workspace_remove_block(
    workspace: *const Workspace,
    trx: *mut Transaction,
    block_id: *const c_char,
) -> bool {
    workspace.as_ref().unwrap().remove(
        trx.as_mut().unwrap(),
        CStr::from_ptr(block_id).to_str().unwrap(),
    )
}
#[no_mangle]
pub unsafe extern "C" fn workspace_exists_block(
    workspace: *const Workspace,
    block_id: *const c_char,
) -> bool {
    workspace
        .as_ref()
        .unwrap()
        .exists(CStr::from_ptr(block_id).to_str().unwrap())
}
