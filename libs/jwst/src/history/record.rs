use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use yrs::{Array, ArrayRef};

use crate::{OctoAnyError, OctoOptionHelper, OctoResult, OctoValue, OctoValueError};

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

impl BlockHistory {
    /// Concern: Uses default values for anything that doesn't match the schema
    #[track_caller]
    pub(crate) fn try_parse_yrs_array(
        yrs_array_ref: ArrayRef,
        read_txn: &impl yrs::ReadTxn,
        block_id: String,
    ) -> Result<Self, OctoAnyError> {
        Ok(Self {
            block_id,
            client: yrs_array_ref
                .get(read_txn, 0)
                .octo_ok_or_any("expected yarray element")
                .and_then(|i| {
                    let st = i.to_string(read_txn);
                    st.parse::<u64>()
                        .octo_map_err_any(format!("failed parsing as u64 for value: {st:?}"))
                })
                .map_err(|e| e.prefix_message("client id at index 0"))?,
            timestamp: yrs_array_ref
                .get(read_txn, 1)
                .octo_ok_or_any("expected yarray element")
                .and_then(|i| {
                    let st = i.to_string(read_txn);
                    st.parse::<u64>()
                        .octo_map_err_any(format!("failed parsing as u64 for value: {st:?}"))
                })
                .map_err(|e| e.prefix_message("timestamp at index 1"))?,
            operation: yrs_array_ref
                .get(read_txn, 2)
                .octo_ok_or_any("expected yarray element")
                .and_then(|i| {
                    let s = OctoValue::from(i)
                        .try_as_string()
                        .octo_map_err_any("expected string")?;

                    HistoryOperation::try_from(s).octo_map_err_any("expected type")
                })
                .map_err(|e| e.prefix_message("operation at index 2"))?,
        })
    }
}
