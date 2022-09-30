pub use serde_json::Value as JsonValue;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use yrs::block::Prelim;

pub struct BlockField {
    r#type: String,
    default: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub enum HistoryOperation {
    Create,
    Update,
    Remove,
    Undefined,
}

impl From<String> for HistoryOperation {
    fn from(str: String) -> Self {
        match str.as_str() {
            "create" => Self::Create,
            "update" => Self::Update,
            "remove" => Self::Remove,
            _ => Self::Undefined,
        }
    }
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct BlockHistory {
    client: i64,
    timestamp: i64,
    block_id: String,
    operation: HistoryOperation,
}

#[derive(Default, Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E", "pos": 0}))]
pub struct InsertChildren {
    pub(crate) block_id: String,
    pub(crate) pos: Option<u32>,
    pub(crate) before: Option<String>,
    pub(crate) after: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E"}))]
pub struct RemoveChildren {
    pub(crate) block_id: String,
}

pub enum BlockContentValue {
    Json(JsonValue),
    Boolean(bool),
    Text(String),
    Number(i64),
}

impl From<JsonValue> for BlockContentValue {
    fn from(json: JsonValue) -> Self {
        Self::Json(json)
    }
}

impl From<bool> for BlockContentValue {
    fn from(bool: bool) -> Self {
        Self::Boolean(bool)
    }
}

impl From<String> for BlockContentValue {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for BlockContentValue {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<i64> for BlockContentValue {
    fn from(num: i64) -> Self {
        Self::Number(num)
    }
}
