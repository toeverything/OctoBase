use log::{debug, info};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use utoipa::ToSchema;
use yrs::{
    block::{Item, ItemContent, ID},
    types::TypePtr,
    updates::decoder::Decode,
    Doc, StateVector, Update,
};

struct ParentMap(HashMap<ID, String>);

impl ParentMap {
    fn parse_parent(name_map: &HashMap<ID, String>, parent: TypePtr) -> Option<String> {
        todo!()
        // match parent {
        //     TypePtr::Unknown => Some("unknown".to_owned()),
        //     TypePtr::Branch(ptr) => {
        //         if let Some(name) = ptr.item_id().and_then(|item_id| name_map.get(&item_id)) {
        //             Some(name.clone())
        //         } else {
        //             None
        //         }
        //     }
        //     TypePtr::Named(name) => Some(name.to_string()),
        //     TypePtr::ID(ptr_id) => {
        //         if let Some(name) = name_map.get(&ptr_id) {
        //             Some(name.clone())
        //         } else {
        //             None
        //         }
        //     }
        // }
    }

    fn from(items: &Vec<&Item>) -> Self {
        todo!()
        // let mut name_map: HashMap<ID, String> = HashMap::new();
        // // println!("{:?}", items);
        // let mut padding_ptr: VecDeque<(&Item, usize)> =
        //     VecDeque::from(items.iter().map(|i| (i.clone(), 0)).collect::<Vec<_>>());

        // while let Some((item, retry)) = padding_ptr.pop_back() {
        //     if retry > 5 {
        //         debug!("retry failed: {:?}, {:?}, {:?}", item, retry, padding_ptr);
        //         break;
        //     }
        //     let (parent, parent_sub) = {
        //         let parent = if item.parent == TypePtr::Unknown {
        //             if let Some((parent, parent_sub)) = item.resolve_parent() {
        //                 Self::parse_parent(&name_map, parent).map(|parent| (parent, parent_sub))
        //             } else {
        //                 Some(("unknown".to_owned(), None))
        //             }
        //         } else {
        //             Self::parse_parent(&name_map, item.parent.clone())
        //                 .map(|parent| (parent, item.parent_sub.clone()))
        //         };

        //         if let Some(parent) = parent {
        //             parent
        //         } else {
        //             padding_ptr.push_front((item, retry + 1));
        //             continue;
        //         }
        //     };

        //     let parent = if let Some(parent_sub) = parent_sub {
        //         format!("{}.{}", parent, parent_sub)
        //     } else {
        //         parent
        //     };

        //     name_map.insert(item.id, parent.clone());
        // }

        // Self(name_map)
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
    let update = []; // doc.encode_state_as_update_v1(&StateVector::default());
    let update = Update::decode_v1(&update).ok()?;

    Some(
        todo!(), // update
                 //     .as_items()
                 //     .iter()
                 //     .map(|i| i.id.client)
                 //     .collect::<HashSet<_>>()
                 //     .into_iter()
                 //     .collect::<Vec<_>>(),
    )
}

pub fn parse_history(doc: &Doc, client: u64) -> Option<Vec<RawHistory>> {
    todo!();
    // let update = doc.encode_state_as_update_v1(&StateVector::default());
    // let update = Update::decode_v1(&update).ok()?;
    // let mut items = update.as_items();

    // let mut histories = vec![];
    // let parent_map = ParentMap::from(&mut items);

    // for item in items {
    //     if let ItemContent::Deleted(_) = item.content {
    //         continue;
    //     }
    //     if let Some(parent) = parent_map.get(&item.id) {
    //         if item.id.client == client || client == 0 {
    //             let id = format!("{}:{}", item.id.clock, item.id.client);
    //             histories.push(RawHistory {
    //                 id,
    //                 parent,
    //                 content: item.content.to_string(),
    //             })
    //         }
    //     } else {
    //         info!("headless id: {:?}", item.id);
    //     }
    // }

    // Some(histories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    #[test]
    fn parse_history_client_test() {
        // let workspace = Workspace::new("test");
        // workspace.with_trx(|mut t| {
        //     let block = t.create("test", "text");
        //     block.set(&mut t.trx, "test", "test");
        // });

        // let doc = workspace.doc();

        // let client = parse_history_client(&doc).unwrap();

        // assert_eq!(client[0], doc.client_id);
    }

    #[test]
    fn parse_history_test() {
        // let workspace = Workspace::new("test");
        // workspace.with_trx(|mut t| {
        //     t.create("test", "text");
        // });
        // let doc = workspace.doc();

        // let history = parse_history(&doc, 0).unwrap();

        // let update = doc.encode_state_as_update_v1(&StateVector::default());
        // let update = Update::decode_v1(&update).unwrap();
        // let mut items = update.as_items();

        // let mut mock_histories: Vec<RawHistory> = vec![];
        // let parent_map = ParentMap::from(&mut items);
        // for item in items {
        //     if let Some(parent) = parent_map.get(&item.id) {
        //         let id = format!("{}:{}", item.id.clock, item.id.client);
        //         mock_histories.push(RawHistory {
        //             id,
        //             parent,
        //             content: item.content.to_string(),
        //         })
        //     }
        // }

        // assert_eq!(history, mock_histories);
    }
}
