pub use serde_json::Value as JsonValue;

use serde::Deserialize;
use utoipa::ToSchema;

pub struct BlockField {
    r#type: String,
    default: String,
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

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E"}))]
pub struct ExistsChildren {
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
