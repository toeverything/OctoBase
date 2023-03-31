use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, info};
use utoipa::ToSchema;
use yrs::{
    block::{Item, ItemContent, ID},
    types::TypePtr,
    updates::decoder::Decode,
    Doc, ReadTxn, StateVector, Transact, Update,
};

struct ParentMap(HashMap<ID, String>);

impl ParentMap {
    fn parse_parent(name_map: &HashMap<ID, String>, parent: TypePtr) -> Option<String> {
        match parent {
            TypePtr::Unknown => Some("unknown".to_owned()),
            TypePtr::Branch(ptr) => ptr
                .item_id()
                .and_then(|item_id| name_map.get(&item_id))
                .cloned(),
            TypePtr::Named(name) => Some(name.to_string()),
            TypePtr::ID(ptr_id) => name_map.get(&ptr_id).cloned(),
        }
    }

    fn from(items: &[&Item]) -> Self {
        let mut name_map: HashMap<ID, String> = HashMap::new();
        let mut padding_ptr: VecDeque<(&Item, usize)> = VecDeque::from(
            items
                .iter()
                .map(|i| (<&Item>::clone(i), 0))
                .collect::<Vec<_>>(),
        );

        while let Some((item, retry)) = padding_ptr.pop_back() {
            if retry > 5 {
                debug!("retry failed: {:?}, {:?}, {:?}", item, retry, padding_ptr);
                break;
            }
            let (parent, parent_sub) = {
                let parent = if item.parent == TypePtr::Unknown {
                    if let Some((parent, parent_sub)) = item.resolve_parent() {
                        Self::parse_parent(&name_map, parent).map(|parent| (parent, parent_sub))
                    } else {
                        Some(("unknown".to_owned(), None))
                    }
                } else {
                    Self::parse_parent(&name_map, item.parent.clone())
                        .map(|parent| (parent, item.parent_sub.clone()))
                };

                if let Some(parent) = parent {
                    parent
                } else {
                    padding_ptr.push_front((item, retry + 1));
                    continue;
                }
            };

            let parent = if let Some(parent_sub) = parent_sub {
                format!("{parent}.{parent_sub}")
            } else {
                parent
            };

            name_map.insert(item.id, parent.clone());
        }

        Self(name_map)
    }

    fn get(&self, id: &ID) -> Option<String> {
        self.0.get(id).cloned()
    }
}

#[derive(Debug, Serialize, ToSchema, PartialEq)]
pub struct RawHistory {
    id: String,
    parent: String,
    content: String,
}

pub fn parse_history_client(doc: &Doc) -> Option<Vec<u64>> {
    let update = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
        .and_then(|update| Update::decode_v1(&update))
        .ok()?;

    Some(
        update
            .as_items()
            .iter()
            .map(|i| i.id.client)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )
}

pub fn parse_history(doc: &Doc, client: u64) -> Option<Vec<RawHistory>> {
    let update = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
        .and_then(|update| Update::decode_v1(&update))
        .ok()?;
    let items = update.as_items();

    let mut histories = vec![];
    let parent_map = ParentMap::from(&items);

    for item in items {
        if let ItemContent::Deleted(_) = item.content {
            continue;
        }
        if let Some(parent) = parent_map.get(&item.id) {
            if item.id.client == client || client == 0 {
                let id = format!("{}:{}", item.id.clock, item.id.client);
                histories.push(RawHistory {
                    id,
                    parent,
                    content: item.content.to_string(),
                })
            }
        } else {
            info!("headless id: {:?}", item.id);
        }
    }

    Some(histories)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Workspace;

    #[test]
    fn parse_history_client_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.create(&mut t.trx, "test", "text").unwrap();
            block.set(&mut t.trx, "test", "test").unwrap();
        });

        let doc = workspace.doc();

        let client = parse_history_client(&doc).unwrap();

        assert_eq!(client[0], doc.client_id());
    }

    #[test]
    fn parse_history_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            t.get_space("test")
                .create(&mut t.trx, "test", "text")
                .unwrap();
        });
        let doc = workspace.doc();

        let history = parse_history(&doc, 0).unwrap();

        let update = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
            .unwrap();
        let update = Update::decode_v1(&update).unwrap();
        let items = update.as_items();

        let mut mock_histories: Vec<RawHistory> = vec![];
        let parent_map = ParentMap::from(&items);
        for item in items {
            if let Some(parent) = parent_map.get(&item.id) {
                let id = format!("{}:{}", item.id.clock, item.id.client);
                mock_histories.push(RawHistory {
                    id,
                    parent,
                    content: item.content.to_string(),
                })
            }
        }

        assert_eq!(history, mock_histories);
    }
}
