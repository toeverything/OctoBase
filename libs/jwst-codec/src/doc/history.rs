use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use serde::Serialize;

use super::{store::StoreRef, *};
use crate::sync::RwLock;

enum ParentNode {
    Root(String),
    Node(Somr<Item>),
    Unknown,
}

#[derive(Clone, Default)]
pub struct HistoryOptions {
    skip: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct StoreHistory {
    store: StoreRef,
    parents: Arc<RwLock<HashMap<Id, Somr<Item>>>>,
}

impl StoreHistory {
    pub(crate) fn new(store: &StoreRef) -> Self {
        Self {
            store: store.clone(),
            ..Default::default()
        }
    }

    pub fn resolve(&self) {
        let store = self.store.read().unwrap();
        self.resolve_with_store(&store);
    }

    pub(crate) fn resolve_with_store(&self, store: &DocStore) {
        let mut parents = self.parents.write().unwrap();

        for node in store.items.values().flat_map(|items| items.iter()) {
            let node = node.as_item();
            if let Some(item) = node.get() {
                parents
                    .entry(item.id)
                    .and_modify(|e| {
                        if *e != node {
                            *e = node.clone();
                        }
                    })
                    .or_insert(node.clone());
            }
        }
    }

    pub fn parse_update(&self, update: &Update, client: u64) -> Vec<History> {
        let store_items = SortedNodes::new(update.structs.iter().collect::<Vec<_>>())
            .filter_map(|n| n.as_item().get().cloned())
            .collect::<Vec<_>>();

        // make items as reference
        let mut store_items = store_items.iter().collect::<Vec<_>>();
        store_items.sort_by(|a, b| a.id.cmp(&b.id));

        self.parse_items(store_items, client)
    }

    pub fn parse_store(&self, client: u64, options: HistoryOptions) -> Vec<History> {
        let store_items = {
            let store = self.store.read().unwrap();
            let mut sort_iter: Box<dyn Iterator<Item = Item>> = Box::new(
                SortedNodes::new(store.items.iter().collect::<Vec<_>>()).filter_map(|n| n.as_item().get().cloned()),
            );
            if let Some(skip) = options.skip {
                sort_iter = Box::new(sort_iter.skip(skip));
            }
            if let Some(limit) = options.limit {
                sort_iter = Box::new(sort_iter.take(limit));
            }

            sort_iter.collect::<Vec<_>>()
        };

        // make items as reference
        let mut store_items = store_items.iter().collect::<Vec<_>>();
        store_items.sort_by(|a, b| a.id.cmp(&b.id));

        self.parse_items(store_items, client)
    }

    fn parse_items(&self, store_items: Vec<&Item>, client: u64) -> Vec<History> {
        let parents = self.parents.read().unwrap();
        let mut histories = vec![];

        for item in store_items {
            if item.deleted() {
                continue;
            }

            if item.id.client == client || client == 0 {
                histories.push(History {
                    id: item.id.to_string(),
                    parent: Self::parse_path(item, &parents),
                    content: Value::try_from(item.content.as_ref())
                        .map(|v| v.to_string())
                        .unwrap_or("unknown".to_owned()),
                })
            }
        }

        histories
    }

    fn parse_path(item: &Item, parents: &HashMap<Id, Somr<Item>>) -> Vec<String> {
        let mut path = Vec::new();
        let mut cur = item.clone();

        while let Some(node) = cur.find_node_with_parent_info() {
            path.push(Self::get_node_name(&node));

            match Self::get_parent(parents, &node.parent) {
                ParentNode::Root(name) => {
                    path.push(name);
                    break;
                }
                ParentNode::Node(parent) => {
                    if let Some(parent) = parent.get() {
                        cur = parent.clone();
                    } else {
                        break;
                    }
                }
                ParentNode::Unknown => {
                    break;
                }
            }
        }

        path.reverse();
        path
    }

    fn get_node_name(item: &Item) -> String {
        if let Some(name) = item.parent_sub.clone() {
            name
        } else {
            let mut curr = item.clone();
            let mut idx = 0;

            while let Some(item) = curr.left_item().get() {
                curr = item.clone();
                idx += 1;
            }

            idx.to_string()
        }
    }

    fn get_parent(parents: &HashMap<Id, Somr<Item>>, parent: &Option<Parent>) -> ParentNode {
        match parent {
            None => ParentNode::Unknown,
            Some(Parent::Type(ptr)) => ptr
                .ty()
                .and_then(|ty| {
                    ty.item
                        .get()
                        .and_then(|i| parents.get(&i.id).map(|p| ParentNode::Node(p.clone())))
                        .or(ty.root_name.clone().map(ParentNode::Root))
                })
                .unwrap_or(ParentNode::Unknown),
            Some(Parent::String(name)) => ParentNode::Root(name.to_string()),
            Some(Parent::Id(id)) => parents
                .get(id)
                .map(|p| ParentNode::Node(p.clone()))
                .unwrap_or(ParentNode::Unknown),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct History {
    pub id: String,
    pub parent: Vec<String>,
    pub content: String,
}

pub(crate) struct SortedNodes<'a> {
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

            let history = StoreHistory::new(&doc.store);

            let update = doc.encode_update().unwrap();

            assert_eq!(
                history.parse_store(0, Default::default()),
                history.parse_update(&update, 0)
            );
        });
    }
}
