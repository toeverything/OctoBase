mod block;
mod dynamic_value;
mod workspace;

pub use block::Block;
pub use dynamic_value::{DynamicValue, DynamicValueMap};
pub use workspace::Workspace;

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type Block;

        #[swift_bridge(associated_to = Block)]
        fn get(self: &Block, block_id: String) -> Option<DynamicValue>;

        #[swift_bridge(associated_to = Block)]
        pub fn children(self: &Block) -> Vec<String>;

        #[swift_bridge(associated_to = Block)]
        pub fn push_children(self: &Block, block: &Block);

        #[swift_bridge(associated_to = Block)]
        pub fn insert_children_at(&self, block: &Block, pos: u32);

        #[swift_bridge(associated_to = Block)]
        pub fn insert_children_before(
            self: &Block,
            block: &Block,
            reference: &str,
        );

        #[swift_bridge(associated_to = Block)]
        pub fn insert_children_after(self: &Block, block: &Block, reference: &str);

        #[swift_bridge(associated_to = Block)]
        pub fn remove_children(self: &Block, block: &Block);

        #[swift_bridge(associated_to = Block)]
        pub fn exists_children(self: &Block, block_id: &str) -> i32;

        #[swift_bridge(associated_to = Block)]
        pub fn parent(self: &Block) -> String;

        #[swift_bridge(associated_to = Block)]
        pub fn updated(self: &Block) -> u64;

        #[swift_bridge(associated_to = Block)]
        pub fn id(self: &Block) -> String;

        #[swift_bridge(associated_to = Block)]
        pub fn flavor(self: &Block) -> String;

        #[swift_bridge(associated_to = Block)]
        pub fn version(self: &Block) -> String;

        #[swift_bridge(associated_to = Block)]
        pub fn created(self: &Block) -> u64;

        #[swift_bridge(associated_to = Block)]
        pub fn set_bool(self: &Block, key: String, value: bool);

        #[swift_bridge(associated_to = Block)]
        pub fn set_string(self: &Block, key: String, value: String);

        #[swift_bridge(associated_to = Block)]
        pub fn set_float(self: &Block, key: String, value: f64);

        #[swift_bridge(associated_to = Block)]
        pub fn set_integer(self: &Block, key: String, value: i64);

        #[swift_bridge(associated_to = Block)]
        pub fn set_null(self: &Block, key: String);

        #[swift_bridge(associated_to = Block)]
        pub fn is_bool(self: &Block, key: String) -> bool;

        #[swift_bridge(associated_to = Block)]
        pub fn is_string(self: &Block, key: String) -> bool;

        #[swift_bridge(associated_to = Block)]
        pub fn is_float(&self, key: String) -> bool;

        #[swift_bridge(associated_to = Block)]
        pub fn is_integer(&self, key: String) -> bool;

        #[swift_bridge(associated_to = Block)]
        pub fn get_bool(&self, key: String) -> Option<i64>;

        #[swift_bridge(associated_to = Block)]
        pub fn get_string(&self, key: String) -> Option<String>;

        #[swift_bridge(associated_to = Block)]
        pub fn get_float(&self, key: String) -> Option<f64>;

        #[swift_bridge(associated_to = Block)]
        pub fn get_integer(&self, key: String) -> Option<i64>;
    }

    extern "Rust" {
        type DynamicValue;
        type DynamicValueMap;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_bool(self: &DynamicValue) -> Option<bool>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_number(self: &DynamicValue) -> Option<f64>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_int(self: &DynamicValue) -> Option<i64>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_string(self: &DynamicValue) -> Option<String>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_map(self: &DynamicValue) -> Option<DynamicValueMap>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_array(self: &DynamicValue) -> Option<Vec<DynamicValue>>;

        #[swift_bridge(associated_to = DynamicValue)]
        fn as_buffer(self: &DynamicValue) -> Option<Vec<u8>>;
    }

    extern "Rust" {
        type Workspace;

        #[swift_bridge(init)]
        fn new(id: String) -> Workspace;

        #[swift_bridge(associated_to = Workspace)]
        fn id(self: &Workspace) -> String;

        fn client_id(self: &Workspace) -> u64;

        #[swift_bridge(associated_to = Workspace)]
        fn get(self: &Workspace, block_id: String) -> Option<Block>;

        #[swift_bridge(associated_to = Workspace)]
        fn create(self: &Workspace, block_id: String, flavor: String) -> Block;
    }
}
