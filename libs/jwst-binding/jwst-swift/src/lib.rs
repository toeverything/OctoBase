use jwst::{Block, Workspace};
use lib0::any::Any;
use std::{
    ffi::{c_void, CStr, CString},
    mem::forget,
    os::raw::c_char,
    ptr,
};
use yrs::{Subscription, Transaction, UpdateEvent};

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type Workspace;
        type Block;

        #[swift_bridge(init)]
        fn new(id: String) -> Workspace;

        #[swift_bridge(associated_to = Workspace)]
        fn get(self: &Workspace, block_id: String) -> Option<Block>;
    }
}
