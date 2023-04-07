use std::collections::HashMap;

use super::*;
use lib0::any::Any;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yrs::{
    types::Value, Array, ArrayPrelim, Map, MapRef, ReadTxn, Transact, Transaction, TransactionMut,
};

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

pub struct PageMeta {
    pub id: String,
    pub favorite: Option<bool>,
    pub is_pivots: Option<bool>,
    pub init: Option<bool>,
    pub sub_page_ids: Vec<String>,
    pub title: Option<String>,
    pub trash: Option<bool>,
    pub trash_date: Option<usize>,
}

impl PageMeta {
    fn get_bool<T: ReadTxn>(trx: &T, map: &MapRef, key: &str) -> Option<bool> {
        map.get(trx, key).and_then(|v| {
            if let Value::Any(Any::Bool(b)) = v {
                Some(b)
            } else {
                None
            }
        })
    }

    fn get_number<T: ReadTxn>(trx: &T, map: &MapRef, key: &str) -> Option<f64> {
        map.get(trx, key).and_then(|v| {
            if let Value::Any(Any::Number(n)) = v {
                Some(n)
            } else {
                None
            }
        })
    }
}

impl From<(&'_ TransactionMut<'_>, MapRef)> for PageMeta {
    fn from((trx, map): (&TransactionMut, MapRef)) -> Self {
        Self {
            id: map.get(trx, "id").unwrap().to_string(trx),
            favorite: Self::get_bool(trx, &map, "favorite"),
            is_pivots: Self::get_bool(trx, &map, "is_pivots"),
            init: Self::get_bool(trx, &map, "init"),
            sub_page_ids: map
                .get(trx, "sub_page_ids")
                .and_then(|v| v.to_yarray())
                .map(|v| v.iter(trx).map(|v| v.to_string(trx)).collect::<Vec<_>>())
                .unwrap_or_default(),
            title: map.get(trx, "title").map(|s| s.to_string(trx)),
            trash: Self::get_bool(trx, &map, "trash"),
            trash_date: Self::get_number(trx, &map, "trash_date")
                .map(|v| v as usize)
                .filter(|v| *v > 0),
        }
    }
}

impl Workspace {
    pub fn metadata(&self) -> WorkspaceMetadata {
        (&self.doc().transact(), self.metadata.clone()).into()
    }

    pub fn pages(&self) -> JwstResult<HashMap<String, PageMeta>> {
        self.retry_with_trx(
            |mut t| {
                let pages = if let Some(pages) = self
                    .metadata
                    .get(&t.trx, "pages")
                    .and_then(|v| v.to_yarray())
                {
                    pages
                } else {
                    self.metadata
                        .insert(&mut t.trx, "pages", ArrayPrelim::default())?
                };

                let pages = pages
                    .iter(&t.trx)
                    .filter_map(|v| v.to_ymap())
                    .map(|v| {
                        let page_meta = PageMeta::from((&t.trx, v));
                        (page_meta.id.clone(), page_meta)
                    })
                    .collect();

                Ok::<_, JwstError>(pages)
            },
            50,
        )
        .and_then(|v| v)
    }
}
