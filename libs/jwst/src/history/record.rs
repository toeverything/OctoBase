use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use yrs::{Array, ArrayRef, ReadTxn};

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

impl From<HistoryOperation> for f64 {
    fn from(op: HistoryOperation) -> f64 {
        match op {
            HistoryOperation::Undefined => 0.0,
            HistoryOperation::Add => 1.0,
            HistoryOperation::Update => 2.0,
            HistoryOperation::Delete => 3.0,
        }
    }
}

impl From<f64> for HistoryOperation {
    fn from(num: f64) -> Self {
        if (0.0..1.0).contains(&num) {
            Self::Undefined
        } else if (1.0..2.0).contains(&num) {
            Self::Add
        } else if (2.0..3.0).contains(&num) {
            Self::Update
        } else if (3.0..4.0).contains(&num) {
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

impl<T: ReadTxn> From<(&'_ T, ArrayRef, String)> for BlockHistory {
    fn from(params: (&'_ T, ArrayRef, String)) -> Self {
        let (trx, array, block_id) = params;
        Self {
            block_id,
            client: array
                .get(trx, 0)
                .and_then(|i| i.to_string(trx).parse::<u64>().ok())
                .unwrap_or_default(),
            timestamp: array
                .get(trx, 1)
                .and_then(|i| i.to_string(trx).parse::<u64>().ok())
                .unwrap_or_default(),
            operation: array
                .get(trx, 2)
                .map(|i| i.to_string(trx))
                .unwrap_or_default()
                .into(),
        }
    }
}
