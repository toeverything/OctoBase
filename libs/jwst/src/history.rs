pub use serde_json::Value as JsonValue;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use yrs::Array;

#[derive(Serialize, Deserialize, ToSchema)]
pub enum HistoryOperation {
    Add,
    Update,
    Delete,
    Undefined,
}

impl From<String> for HistoryOperation {
    fn from(str: String) -> Self {
        match str.as_str() {
            "add" => Self::Add,
            "update" => Self::Update,
            "delete" => Self::Delete,
            _ => Self::Undefined,
        }
    }
}

impl ToString for HistoryOperation {
    fn to_string(&self) -> String {
        match self {
            Self::Add => "add".to_owned(),
            Self::Update => "update".to_owned(),
            Self::Delete => "delete".to_owned(),
            Self::Undefined => "undefined".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct BlockHistory {
    block_id: String,
    client: i64,
    timestamp: i64,
    operation: HistoryOperation,
}

impl From<(Array, String)> for BlockHistory {
    fn from(params: (Array, String)) -> Self {
        let (array, block_id) = params;
        Self {
            block_id,
            client: array
                .get(0)
                .and_then(|i| i.to_string().parse::<i64>().ok())
                .unwrap_or_default(),
            timestamp: array
                .get(1)
                .and_then(|i| i.to_string().parse::<i64>().ok())
                .unwrap_or_default(),
            operation: array
                .get(2)
                .map(|i| i.to_string())
                .unwrap_or_default()
                .into(),
        }
    }
}
