use std::collections::{HashMap, VecDeque};

use serde::Serialize;

use super::*;

/// The ancestor table is a table that records the names of all the ancestors of
/// a node. It is generated every time the history is rebuilt and is used to
/// quickly look up the parent path of a CRDT item. The process of generating
/// this table involves traversing the item nodes and recording their ID as well
/// as their complete name as a parent.
/// TODO: The current implementation is a simple implementation with a lot of
/// room for optimization and should be optimized thereafter
#[derive(Debug)]
struct AncestorTable(HashMap<Id, String>);

impl AncestorTable {
    fn new(items: &[&Item]) -> Self {
        let mut name_map: HashMap<Id, String> = HashMap::new();
        let mut padding_ptr: VecDeque<(&Item, usize)> =
            VecDeque::from(items.iter().map(|i| (<&Item>::clone(i), 0)).collect::<Vec<_>>());

        while let Some((item, retry)) = padding_ptr.pop_back() {
            if retry > 5 {
                debug!("retry failed: {:?}, {:?}, {:?}", item, retry, padding_ptr);
                break;
            }
            let (parent, parent_sub) = {
                let parent = if item.parent.is_none() {
                    if let Some((parent, parent_sub)) = item.resolve_parent() {
                        Self::parse_parent(&name_map, parent).map(|parent| (parent, parent_sub))
                    } else {
                        Some(("unknown".to_owned(), None))
                    }
                } else {
                    Self::parse_parent(&name_map, item.parent.clone()).map(|parent| (parent, item.parent_sub.clone()))
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

    fn parse_parent(name_map: &HashMap<Id, String>, parent: Option<Parent>) -> Option<String> {
        match parent {
            None => Some("unknown".to_owned()),
            Some(Parent::Type(ptr)) => ptr.ty().and_then(|ty| {
                ty.item
                    .get()
                    .and_then(|i| name_map.get(&i.id))
                    .cloned()
                    .or(ty.root_name.clone())
            }),
            Some(Parent::String(name)) => Some(name.to_string()),
            Some(Parent::Id(id)) => name_map.get(&id).cloned(),
        }
    }

    fn get(&self, id: &Id) -> Option<String> {
        self.0.get(id).cloned()
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RawHistory {
    id: String,
    parent: String,
    content: String,
}

struct SortedNodes<'a> {
    nodes: Vec<(&'a Client, &'a VecDeque<Node>)>,
    current: Option<VecDeque<Node>>,
}

impl<'a> SortedNodes<'a> {
    pub fn new(mut nodes: Vec<(&'a Client, &'a VecDeque<Node>)>) -> Self {
        nodes.sort_by(|a, b| b.0.cmp(a.0));
        let current = nodes.pop().map(|(_, v)| v.clone());
        Self { nodes, current }
    }
}

impl Iterator for SortedNodes<'_> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current.as_mut() {
            if let Some(node) = current.pop_back() {
                return Some(node);
            }
        }

        if let Some((_, nodes)) = self.nodes.pop() {
            self.current = Some(nodes.clone());
            self.next()
        } else {
            None
        }
    }
}

impl DocStore {
    pub fn history(&self, client: u64) -> Option<Vec<RawHistory>> {
        let items = SortedNodes::new(self.items.iter().collect::<Vec<_>>())
            .filter_map(|n| n.as_item().get().cloned())
            .collect::<Vec<_>>();
        let mut items = items.iter().collect::<Vec<_>>();
        items.sort_by(|a, b| a.id.cmp(&b.id));

        let mut histories = vec![];
        let parent_map = AncestorTable::new(&items);

        for item in items {
            if item.deleted() {
                continue;
            }
            if let Some(parent) = parent_map.get(&item.id) {
                if item.id.client == client || client == 0 {
                    histories.push(RawHistory {
                        id: item.id.to_string(),
                        parent,
                        content: Value::try_from(item.content.as_ref())
                            .map(|v| v.to_string())
                            .unwrap_or("unknown".to_owned()),
                    })
                }
            } else {
                info!("headless id: {:?}", item.id);
            }
        }

        Some(histories)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_history_client_test() {
        loom_model!({
            let doc = Doc::default();
            let mut map = doc.get_or_create_map("map").unwrap();
            let mut sub_map = doc.create_map().unwrap();
            map.insert("sub_map", sub_map.clone()).unwrap();
            sub_map.insert("key", "value").unwrap();

            assert_eq!(doc.clients()[0], doc.client());
        });
    }

    #[test]
    fn parse_history_test() {
        loom_model!({
            let doc = Doc::default();
            let mut map = doc.get_or_create_map("map").unwrap();
            let mut sub_map = doc.create_map().unwrap();
            map.insert("sub_map", sub_map.clone()).unwrap();
            sub_map.insert("key", "value").unwrap();

            let history = doc.store.read().unwrap().history(0).unwrap();

            let mut update = doc.encode_update().unwrap();
            let items = update
                .iter(StateVector::default())
                .filter_map(|n| n.0.as_item().get().cloned())
                .collect::<Vec<_>>();
            let items = items.iter().collect::<Vec<_>>();

            let mut mock_histories: Vec<RawHistory> = vec![];
            let parent_map = AncestorTable::new(&items);
            for item in items {
                if let Some(parent) = parent_map.get(&item.id) {
                    mock_histories.push(RawHistory {
                        id: item.id.to_string(),
                        parent,
                        content: Value::try_from(item.content.as_ref())
                            .map(|v| v.to_string())
                            .unwrap_or("unknown".to_owned()),
                    })
                }
            }

            assert_eq!(history, mock_histories);
        });
    }
}
