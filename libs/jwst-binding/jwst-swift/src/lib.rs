mod block;
mod dynamic_value;
mod transaction;
mod workspace;

pub use block::Block;
pub use dynamic_value::{DynamicValue, DynamicValueMap};
pub use transaction::Transaction;
pub use workspace::Workspace;

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type Transaction;
    }

    extern "Rust" {
        type Block;

        #[swift_bridge(associated_to = Block)]
        fn get(self: &Block, trx: Transaction<'_>, block_id: String) -> Option<DynamicValue>;
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
