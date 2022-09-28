use super::*;

use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
};
use utoipa::ToSchema;
use yrs::{
    block::{Item, ItemContent, ID},
    types::TypePtr,
    Doc, StateVector,
};

struct ParentMap(HashMap<ID, String>);

impl ParentMap {
    fn from(items: &Vec<&Item>) -> Self {
        let mut parent_map = HashMap::new();

        for item in items {
            if parent_map.contains_key(&item.id) {
                debug!("key {} exists", item.id);
            } else {
                parent_map.insert(
                    item.id.clone(),
                    (item.parent.clone(), item.parent_sub.clone()),
                );
            }
        }

        let mut name_map: HashMap<ID, String> = HashMap::new();
        let mut padding_ptr: VecDeque<(ID, TypePtr, Option<Rc<str>>, usize)> = VecDeque::from(
            parent_map
                .iter()
                .map(|(id, (parent, parent_sub))| {
                    (id.clone(), parent.clone(), parent_sub.clone(), 0)
                })
                .clone()
                .collect::<Vec<_>>(),
        );

        while let Some((map_id, parent, parent_sub, retry)) = padding_ptr.pop_back() {
            if retry > 5 {
                println!(
                    "retry failed: {:?}, {:?}, {:?}, {:?}, {:?}",
                    map_id, parent, parent_sub, retry, padding_ptr
                );
                break;
            }
            let parent = match parent {
                TypePtr::Unknown => "unknown".to_owned(),
                TypePtr::Branch(ptr) => {
                    if let Some(name) = ptr.item_id().and_then(|item_id| name_map.get(&item_id)) {
                        name.clone()
                    } else {
                        padding_ptr.push_front((map_id, parent, parent_sub, retry + 1));
                        continue;
                    }
                }
                TypePtr::Named(name) => name.to_string(),
                TypePtr::ID(ptr_id) => {
                    if let Some(name) = name_map.get(&ptr_id) {
                        name.clone()
                    } else {
                        padding_ptr.push_front((map_id, parent, parent_sub, retry + 1));
                        continue;
                    }
                }
            };

            let parent = if let Some(parent_sub) = &parent_sub {
                format!("{}.{}", parent, parent_sub)
            } else {
                parent
            };

            name_map.insert(map_id, parent.clone());
        }

        Self(name_map)
    }

    fn get(&self, id: &ID) -> Option<String> {
        self.0.get(id).cloned()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct History {
    id: String,
    parent: String,
    content: String,
}

pub fn parse_history(doc: &Doc) -> Option<String> {
    let update = doc.encode_state_as_update_v1(&StateVector::default());
    if let Ok(update) = Update::decode_v1(&update) {
        let items = update.as_items();

        let mut histories = vec![];
        let parent_map = ParentMap::from(&items);

        for item in items {
            if let ItemContent::Deleted(_) = item.content {
                continue;
            }
            if let Some(parent) = parent_map.get(&item.id) {
                let id = format!("{}:{}", item.id.clock, item.id.client);
                histories.push(History {
                    id,
                    parent,
                    content: item.content.to_string(),
                })
            } else {
                println!("cannot find: {:?}", item.id);
            }
        }

        serde_json::to_string(&histories).ok()
    } else {
        None
    }
}
