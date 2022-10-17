use jwst::{Block, Workspace};
use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    ptr,
};
use yrs::{Subscription, Transaction, UpdateEvent};

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
pub unsafe extern "C" fn workspace_new(id: *const c_char) -> *mut Workspace {
    Box::into_raw(Box::new(Workspace::new(
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
) -> *mut Block {
    let block = workspace
        .as_ref()
        .unwrap()
        .get(CStr::from_ptr(block_id).to_str().unwrap());

    if let Some(block) = block {
        Box::leak(Box::new(block))
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn workspace_create_block(
    workspace: *const Workspace,
    block_id: *const c_char,
    flavor: *const c_char,
) -> *mut Block {
    let block_id = CStr::from_ptr(block_id).to_str().unwrap();
    let flavor = CStr::from_ptr(flavor).to_str().unwrap();
    let block = workspace
        .as_ref()
        .unwrap()
        .get_trx()
        .create(block_id, flavor);

    Box::into_raw(Box::new(block))
}

#[no_mangle]
pub unsafe extern "C" fn workspace_remove_block(
    workspace: *const Workspace,
    block_id: *const c_char,
) -> bool {
    workspace
        .as_ref()
        .unwrap()
        .get_trx()
        .remove(CStr::from_ptr(block_id).to_str().unwrap())
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

#[no_mangle]
pub unsafe extern "C" fn workspace_observe(
    workspace: *mut Workspace,
    env: *mut c_void,
    func: extern "C" fn(*mut c_void, *const Transaction, *const UpdateEvent),
) -> *mut Subscription<UpdateEvent> {
    Box::into_raw(Box::new(
        workspace
            .as_mut()
            .unwrap()
            .observe(move |tx, upd| func(env, tx, upd)),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn workspace_unobserve(subscription: *mut Subscription<UpdateEvent>) {
    drop(Box::from_raw(subscription))
}
