pub use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Deserialize, PartialEq, Debug, ToSchema)]
pub struct Workspace {
    pub(super) blocks: HashMap<String, Block>,
    pub(super) updated: HashMap<String, BlockRawHistory>,
}

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
#[schema(example = json!({
    "sys_id": "0",
    "sys:flavour": "affine:text",
    "sys:created": 946684800000_u64,
    "sys:children": ["block1", "block2"],
    "prop:text": "123",
    "prop:color": "#ff0000",
}))]
pub struct Block {
    #[serde(rename = "sys:flavour")]
    flavour: String,
    #[serde(rename = "sys:created")]
    created: u64,
    #[serde(rename = "sys:children")]
    children: Vec<String>,
}

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
#[schema(example = json!([12345, 946684800000_u64, "add"]))]
pub struct BlockRawHistory(u64, u64, String);

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"Push": "jwstRf4rMzua7E"}))]

pub enum InsertChildren {
    Push(String),
    InsertBefore { id: String, before: String },
    InsertAfter { id: String, after: String },
    InsertAt { id: String, pos: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example=json!({
    "hookEndpoint": "http://localhost:3000/api/hooks"
}))]
pub struct SubscribeWorkspace {
    #[serde(rename = "hookEndpoint")]
    pub hook_endpoint: String,
}
