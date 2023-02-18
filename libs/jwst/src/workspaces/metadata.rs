use std::collections::HashMap;

use lib0::any::Any;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yrs::{Map, MapRef, Transaction};

#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub name: Option<String>,
}

impl From<(&'_ Transaction<'_>, MapRef)> for WorkspaceMetadata {
    fn from((trx, map): (&Transaction, MapRef)) -> Self {
        Self {
            name: map.get(trx, "name").map(|s| s.to_string(trx)),
        }
    }
}

impl From<WorkspaceMetadata> for Any {
    fn from(val: WorkspaceMetadata) -> Self {
        let mut map = HashMap::new();
        if let Some(name) = val.name {
            map.insert("name".to_owned(), name.into());
        }
        Any::Map(map.into())
    }
}
