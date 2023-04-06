mod block;
mod dynamic_value;
mod storage;
mod workspace;

pub use block::Block;
pub use dynamic_value::{DynamicValue, DynamicValueMap};
use jwst::JwstError;
pub use storage::Storage;
pub use workspace::Workspace;

type JwstWorkSpaceResult = Result<Workspace, JwstError>;

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type Block;

        fn get(self: &Block, block_id: String) -> Option<DynamicValue>;

        pub fn children(self: &Block) -> Vec<String>;

        pub fn push_children(self: &Block, block: &Block);

        pub fn insert_children_at(&self, block: &Block, pos: u32);

        pub fn insert_children_before(self: &Block, block: &Block, reference: &str);

        pub fn insert_children_after(self: &Block, block: &Block, reference: &str);

        pub fn remove_children(self: &Block, block: &Block);

        pub fn exists_children(self: &Block, block_id: &str) -> i32;

        pub fn parent(self: &Block) -> String;

        pub fn updated(self: &Block) -> u64;

        pub fn id(self: &Block) -> String;

        pub fn flavour(self: &Block) -> String;

        pub fn created(self: &Block) -> u64;

        pub fn set_bool(self: &Block, key: String, value: bool);

        pub fn set_string(self: &Block, key: String, value: String);

        pub fn set_float(self: &Block, key: String, value: f64);

        pub fn set_integer(self: &Block, key: String, value: i64);

        pub fn set_null(self: &Block, key: String);

        pub fn is_bool(self: &Block, key: String) -> bool;

        pub fn is_string(self: &Block, key: String) -> bool;

        pub fn is_float(&self, key: String) -> bool;

        pub fn is_integer(&self, key: String) -> bool;

        pub fn get_bool(&self, key: String) -> Option<i64>;

        pub fn get_string(&self, key: String) -> Option<String>;

        pub fn get_float(&self, key: String) -> Option<f64>;

        pub fn get_integer(&self, key: String) -> Option<i64>;
    }

    extern "Rust" {
        type DynamicValue;
        type DynamicValueMap;

        fn as_bool(self: &DynamicValue) -> Option<bool>;

        fn as_number(self: &DynamicValue) -> Option<f64>;

        fn as_int(self: &DynamicValue) -> Option<i64>;

        fn as_string(self: &DynamicValue) -> Option<String>;

        fn as_map(self: &DynamicValue) -> Option<DynamicValueMap>;

        fn as_array(self: &DynamicValue) -> Option<Vec<DynamicValue>>;

        fn as_buffer(self: &DynamicValue) -> Option<Vec<u8>>;
    }

    extern "Rust" {
        type Workspace;

        #[swift_bridge(init)]
        fn new(id: String) -> Workspace;

        fn id(self: &Workspace) -> String;

        fn client_id(self: &Workspace) -> u64;

        fn get(self: &Workspace, block_id: String) -> Option<Block>;

        fn create(self: &Workspace, block_id: String, flavour: String) -> Block;

        fn search(self: &Workspace, query: String) -> String;

        fn get_blocks_by_flavour(self: &Workspace, flavour: &str) -> Vec<Block>;

        fn get_search_index(self: &Workspace) -> Vec<String>;

        fn set_search_index(self: &Workspace, fields: Vec<String>) -> bool;
    }

    extern "Rust" {
        type JwstWorkSpaceResult;
    }

    extern "Rust" {
        type Storage;

        #[swift_bridge(init)]
        fn new(path: String) -> Storage;

        #[swift_bridge(init)]
        fn new_with_log_level(path: String, level: String) -> Storage;

        fn error(self: &Storage) -> Option<String>;

        fn is_offline(self: &Storage) -> bool;

        fn is_initialized(self: &Storage) -> bool;

        fn is_syncing(self: &Storage) -> bool;

        fn is_finished(self: &Storage) -> bool;

        fn is_error(self: &Storage) -> bool;

        fn get_sync_state(self: &Storage) -> String;

        fn connect(self: &mut Storage, workspace_id: String, remote: String) -> Option<Workspace>;

        fn sync(self: &Storage, workspace_id: String, remote: String) -> JwstWorkSpaceResult;
    }
}
