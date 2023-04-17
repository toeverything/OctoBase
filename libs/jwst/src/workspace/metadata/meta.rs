use std::collections::HashMap;

use super::*;
use lib0::any::Any;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yrs::{ArrayPrelim, ArrayRef, Map, MapRef, ReadTxn, Transact, TransactionMut};

#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceMetadata {
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub search_index: Vec<String>,
}

impl<T: ReadTxn> From<(&T, MapRef)> for WorkspaceMetadata {
    fn from((trx, map): (&T, MapRef)) -> Self {
        Self {
            name: map
                .get(trx, constants::metadata::NAME)
                .map(|s| s.to_string(trx)),
            avatar: map.get(trx, "avatar").map(|s| s.to_string(trx)),
            search_index: match map.get(trx, constants::metadata::SEARCH_INDEX) {
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
            map.insert(constants::metadata::NAME.to_owned(), name.into());
        }
        if let Some(avatar) = val.avatar {
            map.insert(constants::metadata::AVATAR.to_owned(), avatar.into());
        }
        map.insert(
            constants::metadata::SEARCH_INDEX.to_owned(),
            val.search_index.into(),
        );
        Any::Map(map.into())
    }
}

impl Workspace {
    pub fn metadata(&self) -> WorkspaceMetadata {
        (&self.doc().transact(), self.metadata.clone()).into()
    }

    pub fn pages(&self, trx: &mut TransactionMut) -> JwstResult<ArrayRef> {
        Ok(
            if let Some(pages) = self.metadata.get(trx, "pages").and_then(|v| v.to_yarray()) {
                pages
            } else {
                self.metadata.insert(trx, "pages", ArrayPrelim::default())?
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::Doc;

    #[test]
    fn test_workspace_metadata() {
        let doc = Doc::new();
        let ws = Workspace::from_doc(doc, "test");
        ws.set_search_index(vec!["test1".to_string(), "test2".to_string()])
            .unwrap();
        ws.with_trx(|mut t| {
            t.set_metadata(constants::metadata::NAME, "test_name")
                .unwrap();
        });
        ws.with_trx(|mut t| {
            t.set_metadata(constants::metadata::AVATAR, "test_avatar")
                .unwrap();
        });
        assert_eq!(
            ws.metadata(),
            WorkspaceMetadata {
                name: Some("test_name".to_string()),
                avatar: Some("test_avatar".to_string()),
                search_index: vec!["test1".to_string(), "test2".to_string()],
            }
        );
    }
}
