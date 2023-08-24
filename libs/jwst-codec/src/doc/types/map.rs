use std::collections::HashMap;

use super::*;
use crate::{
    doc::{AsInner, Node, Parent, YTypeRef},
    impl_type,
    sync::Arc,
    Content, JwstCodecResult,
};

impl_type!(Map);

pub(crate) trait MapType: AsInner<Inner = YTypeRef> {
    fn insert(&mut self, key: impl AsRef<str>, value: impl Into<Content>) -> JwstCodecResult {
        if let Some((mut store, mut ty)) = self.as_inner().write() {
            let left = ty.map.as_ref().and_then(|map| {
                map.get(key.as_ref())
                    .and_then(|struct_info| struct_info.left())
                    .map(|l| l.as_item())
            });

            let item = store.create_item(
                value.into(),
                left.unwrap_or(Somr::none()),
                Somr::none(),
                Some(Parent::Type(self.as_inner().clone())),
                Some(key.as_ref().into()),
            );
            store.integrate(Node::Item(item), 0, Some(&mut ty))?;
        }

        Ok(())
    }

    fn keys(&self) -> Vec<String> {
        if let Some(ty) = self.as_inner().ty() {
            ty.map.as_ref().map_or(Vec::new(), |map| map.keys().cloned().collect())
        } else {
            vec![]
        }
    }

    fn get(&self, key: impl AsRef<str>) -> Option<Arc<Content>> {
        self.as_inner().ty().and_then(|ty| {
            ty.map
                .as_ref()
                .and_then(|map| map.get(key.as_ref()))
                .filter(|struct_info| !struct_info.deleted())
                .map(|struct_info| struct_info.as_item().get().unwrap().content.clone())
        })
    }

    fn get_all(&self) -> HashMap<String, Arc<Content>> {
        let mut ret = HashMap::default();

        if let Some(ty) = self.as_inner().ty() {
            if let Some(map) = ty.map.as_ref() {
                for key in map.keys() {
                    if let Some(content) = self.get(key) {
                        ret.insert(key.clone(), content);
                    }
                }
            }
        }

        ret
    }

    fn contains_key(&self, key: impl AsRef<str>) -> bool {
        if let Some(ty) = self.as_inner().ty() {
            ty.map
                .as_ref()
                .and_then(|map| map.get(key.as_ref()))
                .map(|struct_info| !struct_info.deleted())
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn remove(&mut self, key: impl AsRef<str>) -> bool {
        if let Some((mut store, mut ty)) = self.as_inner().write() {
            let node = ty.map.as_ref().and_then(|map| map.get(key.as_ref()));
            if let Some(node) = node {
                if let Some(item) = node.as_item().get() {
                    store.delete_set.add(item.id.client, item.id.clock, item.len());
                    DocStore::delete_item(item, Some(&mut ty));
                    return true;
                }
            }
        }

        false
    }

    fn len(&self) -> u64 {
        self.as_inner()
            .ty()
            .map(|ty| {
                ty.map
                    .as_ref()
                    .map_or(0, |map| map.values().filter(|v| !v.deleted()).count()) as u64
            })
            .unwrap_or(0)
    }

    fn iter(&self) -> MapIterator {
        let inner = self.as_inner().ty().unwrap();
        let map = inner.map.as_ref().map(|map| {
            map.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<(String, Node)>>()
        });

        MapIterator {
            nodes: map.unwrap_or(vec![]),
            index: 0,
        }
    }
}

pub struct MapIterator {
    pub(super) nodes: Vec<(String, Node)>,
    pub(super) index: usize,
}

impl Iterator for MapIterator {
    type Item = (String, Value);

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.nodes.len();
        if self.index >= len {
            return None;
        }

        while self.index < len {
            let (name, node) = self.nodes[self.index].clone();
            self.index += 1;
            if node.deleted() {
                continue;
            }

            if let Some(item) = node.as_item().get() {
                return item.content.as_ref().try_into().ok().map(|item| (name, item));
            } else {
                continue;
            }
        }

        None
    }
}

impl MapType for Map {}

impl Map {
    pub fn iter(&self) -> MapIterator {
        MapType::iter(self)
    }

    pub fn insert<K: AsRef<str>, V: Into<Value>>(&mut self, key: K, value: V) -> JwstCodecResult {
        MapType::insert(self, key, value.into())
    }

    pub fn keys(&self) -> Vec<String> {
        MapType::keys(self)
    }

    #[inline]
    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<Value> {
        if let Some(content) = MapType::get(self, key) {
            // TODO: rewrite to content.read(&mut [Any])
            return match content.as_ref() {
                Content::Any(any) => return any.first().map(|any| Value::Any(any.clone())),
                _ => content.as_ref().try_into().map_or_else(|_| None, Some),
            };
        }

        None
    }

    #[inline]
    pub fn remove<K: AsRef<str>>(&mut self, key: K) -> bool {
        MapType::remove(self, key)
    }

    #[inline]
    pub fn contains_key<K: AsRef<str>>(&self, key: K) -> bool {
        MapType::contains_key(self, key)
    }

    pub fn len(&self) -> u64 {
        MapType::len(self)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl serde::Serialize for Map {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.len() as usize))?;
        for (key, value) in self.iter() {
            map.serialize_entry(&key, &value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loom_model, Any, Doc};

    #[test]
    fn test_map_basic() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        loom_model!({
            let doc = Doc::with_options(options.clone());
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1", "value").unwrap();
            assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value".to_string())));
            assert!(!map.contains_key("nonexistent_key"));
            assert_eq!(map.len(), 1);
            assert!(map.contains_key("1"));
            map.remove("1");
            assert!(!map.contains_key("1"));
            assert_eq!(map.len(), 0);
        });
    }

    #[test]
    fn test_map_equal() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        loom_model!({
            let doc = Doc::with_options(options.clone());
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1", "value").unwrap();
            map.insert("2", false).unwrap();

            let binary = doc.encode_update_v1().unwrap();
            let new_doc = Doc::new_from_binary_with_options(binary, options.clone()).unwrap();
            let map = new_doc.get_or_create_map("map").unwrap();
            assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value".to_string())));
            assert_eq!(map.get("2").unwrap(), Value::Any(Any::False));
            assert_eq!(map.len(), 2);
        });
    }

    // #[test]
    // fn test_map_iter() {
    //     let options = DocOptions {
    //         client: Some(rand::random()),
    //         guid: Some(nanoid::nanoid!()),
    //     };

    //     loom_model!({
    //         let doc = Doc::with_options(options.clone());
    //         let mut map = doc.get_or_create_map("map").unwrap();
    //         map.insert("1", "value1").unwrap();
    //         map.insert("2", "value2").unwrap();
    //         let iter = map.iter();
    //         assert_eq!(iter.count(), 2);

    //         let iter = map.iter();
    //         let mut vec: Vec<_> = iter.collect();
    //         vec.sort_by(|a, b| a.id.cmp(&b.id));
    //         assert_eq!(
    //             vec[0].content,
    //             Arc::new(Content::Any(vec![Any::String("value1".to_string())]))
    //         );
    //         assert_eq!(
    //             vec[1].content,
    //             Arc::new(Content::Any(vec![Any::String("value2".to_string())]))
    //         );
    //     });
    // }

    #[test]
    fn test_map_re_encode() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        loom_model!({
            let binary = {
                let doc = Doc::with_options(options.clone());
                let mut map = doc.get_or_create_map("map").unwrap();
                map.insert("1", "value1").unwrap();
                map.insert("2", "value2").unwrap();
                doc.encode_update_v1().unwrap()
            };

            {
                let doc = Doc::new_from_binary_with_options(binary, options.clone()).unwrap();
                let map = doc.get_or_create_map("map").unwrap();
                assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value1".to_string())));
                assert_eq!(map.get("2").unwrap(), Value::Any(Any::String("value2".to_string())));
            }
        });
    }
}
