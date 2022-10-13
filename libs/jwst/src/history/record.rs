pub use serde_json::Value as JsonValue;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use yrs::Array;

#[derive(Serialize, Deserialize, ToSchema, Debug, PartialEq)]
pub enum HistoryOperation {
    Undefined,
    Add,
    Update,
    Delete,
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

impl Into<f64> for HistoryOperation {
    fn into(self) -> f64 {
        match self {
            Self::Undefined => 0.0,
            Self::Add => 1.0,
            Self::Update => 2.0,
            Self::Delete => 3.0,
        }
    }
}

impl From<f64> for HistoryOperation {
    fn from(num: f64) -> Self {
        if num >= 0.0 && num < 1.0 {
            Self::Undefined
        } else if num >= 1.0 && num < 2.0 {
            Self::Add
        } else if num >= 2.0 && num < 3.0 {
            Self::Update
        } else if num >= 3.0 && num < 4.0 {
            Self::Delete
        } else {
            Self::Undefined
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

#[derive(Serialize, Deserialize, ToSchema, Debug, PartialEq)]
pub struct BlockHistory {
    pub block_id: String,
    pub client: u64,
    pub timestamp: u64,
    pub operation: HistoryOperation,
}

impl From<(Array, String)> for BlockHistory {
    fn from(params: (Array, String)) -> Self {
        let (array, block_id) = params;
        Self {
            block_id,
            client: array
                .get(0)
                .and_then(|i| i.to_string().parse::<u64>().ok())
                .unwrap_or_default(),
            timestamp: array
                .get(1)
                .and_then(|i| i.to_string().parse::<u64>().ok())
                .unwrap_or_default(),
            operation: array
                .get(2)
                .map(|i| i.to_string())
                .unwrap_or_default()
                .into(),
        }
    }
}
