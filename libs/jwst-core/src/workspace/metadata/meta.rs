use super::*;
use jwst_codec::{Any, Array, Map};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceMetadata {
    pub name: Option<String>,
    pub avatar: Option<String>,
}

impl From<Map> for WorkspaceMetadata {
    fn from(map: Map) -> Self {
        Self {
            name: map.get(constants::metadata::NAME).map(|s| s.to_string()),
            avatar: map.get(constants::metadata::AVATAR).map(|s| s.to_string()),
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
        Any::Object(map.into())
    }
}

impl Workspace {
    pub fn metadata(&self) -> WorkspaceMetadata {
        self.metadata.clone().into()
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) -> JwstResult<()> {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Null | Any::Undefined => {
                self.metadata.remove(&key);
            }
            value => {
                self.metadata.insert(key, value)?;
            }
        }

        Ok(())
    }

    pub fn pages(&mut self) -> JwstResult<Array> {
        Ok(
            if let Some(pages) = self.metadata.get("pages").and_then(|v| v.to_array()) {
                pages
            } else {
                let array = self.doc.create_array()?;
                self.metadata.insert("pages", array.clone())?;
                array
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jwst_codec::Doc;

    #[test]
    fn test_workspace_metadata() {
        let doc = Doc::default();
        let mut ws = Workspace::from_doc(doc, "test").unwrap();

        ws.set_metadata(constants::metadata::NAME, "test_name")
            .unwrap();
        ws.set_metadata(constants::metadata::AVATAR, "test_avatar")
            .unwrap();

        assert_eq!(
            ws.metadata(),
            WorkspaceMetadata {
                name: Some("test_name".to_string()),
                avatar: Some("test_avatar".to_string()),
            }
        );
    }
}
