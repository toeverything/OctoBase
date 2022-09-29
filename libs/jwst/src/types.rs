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

pub enum BlockContentValue {
    Json(JsonValue),
    Text(String),
}

impl From<JsonValue> for BlockContentValue {
    fn from(json: JsonValue) -> Self {
        Self::Json(json)
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
