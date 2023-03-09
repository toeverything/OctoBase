use std::collections::HashMap;

use lib0::any::Any;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yrs::{Map, MapRef, Transaction};

pub const SEARCH_INDEX: &str = "search_index";

#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub name: Option<String>,
    // pub avatar: Option<String>,
    pub search_index: Vec<String>,
}

impl From<(&'_ Transaction<'_>, MapRef)> for WorkspaceMetadata {
    fn from((trx, map): (&Transaction, MapRef)) -> Self {
        Self {
            name: map.get(trx, "name").map(|s| s.to_string(trx)),
            // avatar: map.get(trx, "avatar").map(|s| s.to_string(trx)),
            search_index: match map.get(trx, SEARCH_INDEX) {
                Some(value) => serde_json::from_str::<Vec<String>>(&value.to_string(trx)).unwrap(),
                None => vec!["title".to_string(), "text".to_string()],
            },
        }
    }
}

impl From<WorkspaceMetadata> for Any {
    fn from(val: WorkspaceMetadata) -> Self {
        let mut map = HashMap::new();
        if let Some(name) = val.name {
            map.insert("name".to_owned(), name.into());
        }
        // if let Some(avatar) = val.avatar {
        //     map.insert("avatar".to_owned(), avatar.into());
        // }
        map.insert(SEARCH_INDEX.to_owned(), val.search_index.into());
        Any::Map(map.into())
    }
}
