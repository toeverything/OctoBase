use jwst::{Block, Workspace};
use lib0::any::Any;
use std::{
    ffi::{c_void, CStr, CString},
    mem::forget,
    os::raw::c_char,
    ptr,
};
use yrs::{Subscription, Transaction, UpdateEvent};

#[no_mangle]
pub unsafe extern "C" fn block_new(
    workspace: *const Workspace,
    block_id: *const c_char,
    flavour: *const c_char,
    operator: u64,
) -> *mut Block {
    Box::into_raw(Box::new(Block::new(
        workspace.as_ref().unwrap(),
        CStr::from_ptr(block_id).to_str().unwrap(),
        CStr::from_ptr(flavour).to_str().unwrap(),
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
pub unsafe extern "C" fn block_get_flavour(block: *const Block) -> *mut c_char {
    CString::new(block.as_ref().unwrap().flavour())
        .unwrap()
        .into_raw()
}

#[repr(C)]
pub struct BlockChildren {
    len: usize,
    data: *mut *mut c_char,
}

#[no_mangle]
pub unsafe extern "C" fn block_get_children(block: *const Block) -> *mut BlockChildren {
    let mut child: Box<[*mut c_char]> = block
        .as_ref()
        .unwrap()
        .children()
        .into_iter()
        .map(|id| CString::new(id).unwrap().into_raw())
        .collect();
    let len = child.len();
    let data = child.as_mut_ptr();

    forget(child);

    Box::into_raw(BlockChildren { len, data }.into())
}

#[no_mangle]
pub unsafe extern "C" fn block_push_children(
    block: *const Block,
    trx: *mut Transaction,
    child: *const Block,
) {
    let block = block.as_ref().unwrap();
    let trx = trx.as_mut().unwrap();
    let child = child.as_ref().unwrap();

    block.push_children(trx, child);
}

#[no_mangle]
pub unsafe extern "C" fn block_insert_children_at(
    block: *const Block,
    trx: *mut Transaction,
    child: *const Block,
    pos: u32,
) {
    let block = block.as_ref().unwrap();
    let trx = trx.as_mut().unwrap();
    let child = child.as_ref().unwrap();

    block.insert_children_at(trx, child, pos);
}

#[no_mangle]
pub unsafe extern "C" fn block_insert_children_before(
    block: *const Block,
    trx: *mut Transaction,
    child: *const Block,
    reference: *const c_char,
) {
    let block = block.as_ref().unwrap();
    let trx = trx.as_mut().unwrap();
    let child = child.as_ref().unwrap();
    let reference = CStr::from_ptr(reference).to_str().unwrap();

    block.insert_children_before(trx, child, reference);
}

#[no_mangle]
pub unsafe extern "C" fn block_insert_children_after(
    block: *const Block,
    trx: *mut Transaction,
    child: *const Block,
    reference: *const c_char,
) {
    let block = block.as_ref().unwrap();
    let trx = trx.as_mut().unwrap();
    let child = child.as_ref().unwrap();
    let reference = CStr::from_ptr(reference).to_str().unwrap();

    block.insert_children_after(trx, child, reference);
}

#[no_mangle]
pub unsafe extern "C" fn block_children_destroy(children: *mut BlockChildren) {
    let children = children.as_mut().unwrap();
    let vec = Vec::from_raw_parts(children.data, children.len, children.len);

    for item in vec {
        let str = CString::from_raw(item);
        drop(str)
    }
}

pub const BLOCK_TAG_NUM: i8 = 1;
pub const BLOCK_TAG_INT: i8 = 2;
pub const BLOCK_TAG_BOOL: i8 = 3;
pub const BLOCK_TAG_STR: i8 = 4;

#[repr(C)]
pub struct BlockContent {
    tag: i8,
    value: BlockValue,
}
#[repr(C)]
union BlockValue {
    num: f64,
    int: i64,
    bool: bool,
    str: *mut c_char,
}

impl From<BlockContent> for Any {
    fn from(content: BlockContent) -> Self {
        unsafe {
            match content.tag {
                BLOCK_TAG_BOOL => Self::Bool(content.value.bool),
                BLOCK_TAG_NUM => Self::Number(content.value.num),
                BLOCK_TAG_INT => Self::BigInt(content.value.int),
                BLOCK_TAG_STR => Self::String(
                    CString::from_raw(content.value.str)
                        .into_string()
                        .unwrap()
                        .into_boxed_str(),
                ),
                _ => unreachable!("invalid tag value"),
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn block_get_content(
    block: *const Block,
    key: *const c_char,
) -> *mut BlockContent {
    let res = block
        .as_ref()
        .unwrap()
        .get(CStr::from_ptr(key).to_str().unwrap());

    if let Some(res) = res {
        match res {
            Any::String(str) => Box::into_raw(
                BlockContent {
                    tag: BLOCK_TAG_STR,
                    value: BlockValue {
                        str: CString::new(str.to_string()).unwrap().into_raw(),
                    },
                }
                .into(),
            ),
            Any::Bool(b) => Box::into_raw(
                BlockContent {
                    tag: BLOCK_TAG_BOOL,
                    value: BlockValue { bool: b },
                }
                .into(),
            ),
            Any::Number(num) => Box::into_raw(
                BlockContent {
                    tag: BLOCK_TAG_NUM,
                    value: BlockValue { num },
                }
                .into(),
            ),
            Any::BigInt(int) => Box::into_raw(
                BlockContent {
                    tag: BLOCK_TAG_INT,
                    value: BlockValue { int },
                }
                .into(),
            ),
            Any::Null | Any::Undefined | Any::Array(_) | Any::Buffer(_) | Any::Map(_) => {
                ptr::null_mut()
            }
        }
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn block_set_content(
    block: *mut Block,
    key: *const c_char,
    trx: *mut Transaction,
    content: BlockContent,
) {
    let block = block.as_mut().unwrap();
    let trx = trx.as_mut().unwrap();
    let key = CStr::from_ptr(key).to_str().unwrap();

    let value = content;

    block.set(trx, key, value);
}

#[no_mangle]
pub unsafe extern "C" fn block_content_destroy(content: *mut BlockContent) {
    let content = Box::from_raw(content);
    if content.tag == BLOCK_TAG_STR {
        let str = content.value.str;
        drop(CString::from_raw(str));
    }
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
    flavour: *const c_char,
) -> *mut Block {
    let block_id = CStr::from_ptr(block_id).to_str().unwrap();
    let flavour = CStr::from_ptr(flavour).to_str().unwrap();
    let block = workspace
        .as_ref()
        .unwrap()
        .with_trx(|t| t.create(block_id, flavour));

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
        .with_trx(|mut t| t.remove(CStr::from_ptr(block_id).to_str().unwrap()))
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
pub unsafe extern "C" fn trx_commit(trx: *mut Transaction) {
    drop(Box::from_raw(trx))
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
