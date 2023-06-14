use crate::doc::{AsInner, ItemBuilder, ItemRef, Parent, StructInfo, YTypeRef};
use crate::{impl_type, Content, JwstCodecResult};
use std::collections::HashMap;
use std::sync::Arc;

impl_type!(Map);

pub(crate) trait MapType: AsInner<Inner = YTypeRef> {
    fn insert(&mut self, key: String, value: Content) -> JwstCodecResult {
        let mut inner = self.as_inner().write().unwrap();
        let left = inner.map.as_ref().and_then(|map| {
            map.get(key.as_str())
                .and_then(|struct_info| struct_info.left())
        });
        if let Some(store) = inner.store.upgrade() {
            let mut store = store.write().unwrap();
            let new_item_id = (store.client(), store.get_state(store.client())).into();
            let item = Arc::new(
                ItemBuilder::new()
                    .id(new_item_id)
                    .left(left)
                    .content(value)
                    .parent(Some(Parent::Type(self.as_inner().clone())))
                    .parent_sub(Some(key))
                    .build(),
            );
            store.integrate(StructInfo::Item(item), 0, Some(&mut inner))?;
        }

        Ok(())
    }

    fn get(&self, key: String) -> Option<&Content> {
        let inner = self.as_inner().read().unwrap();
        inner
            .map
            .as_ref()
            .and_then(|map| map.get(key.as_str()))
            .filter(|struct_info| !struct_info.deleted())
            .and_then(|struct_info| struct_info.as_item())
            .map(|item| {
                let ptr = Arc::as_ptr(&item.content);
                unsafe { &*ptr }
            })
    }

    fn get_all(&self) -> HashMap<String, Option<&Content>> {
        let mut ret = HashMap::default();
        let inner = self.as_inner().read().unwrap();
        if let Some(map) = inner.map.as_ref() {
            for key in map.keys() {
                ret.insert(key.clone(), self.get(key.to_string()));
            }
        }

        ret
    }

    fn contains_key(&self, key: String) -> bool {
        let inner = self.as_inner().read().unwrap();
        inner
            .map
            .as_ref()
            .and_then(|map| map.get(key.as_str()))
            .map(|struct_info| !struct_info.deleted())
            .unwrap_or(false)
    }

    fn remove(&mut self, key: String) {
        let inner = self.as_inner().write().unwrap();
        if let Some(struct_info) = inner.map.as_ref().and_then(|map| map.get(key.as_str())) {
            struct_info.delete();
        }
    }

    fn len(&self) -> u64 {
        let inner = self.as_inner().write().unwrap();
        inner
            .map
            .as_ref()
            .map_or(0, |map| map.values().filter(|v| !v.deleted()).count()) as u64
    }

    fn iter(&self) -> MapIterator {
        let inner = self.as_inner().read().unwrap();
        let map = inner
            .map
            .as_ref()
            .map(|map| map.values().cloned().collect::<Vec<StructInfo>>());

        MapIterator {
            struct_info_vec: map.unwrap_or(vec![]),
            index: 0,
        }
    }
}

pub struct MapIterator {
    pub(super) struct_info_vec: Vec<StructInfo>,
    pub(super) index: usize,
}

impl Iterator for MapIterator {
    type Item = ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.struct_info_vec.len();
        if self.index >= len {
            return None;
        }

        while self.index < len {
            let struct_info = self.struct_info_vec[self.index].clone();
            self.index += 1;
            if struct_info.deleted() {
                continue;
            }

            return struct_info.as_item();
        }

        None
    }
}

impl MapType for Map {}

impl Map {
    pub fn insert<K: AsRef<str>, V: Into<Content>>(&mut self, key: K, value: V) -> JwstCodecResult {
        MapType::insert(self, key.as_ref().into(), value.into())
    }

    #[inline]
    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<&Content> {
        MapType::get(self, key.as_ref().into())
    }

    #[inline]
    pub fn remove<K: AsRef<str>>(&mut self, key: K) {
        MapType::remove(self, key.as_ref().into())
    }

    #[inline]
    pub fn contains_key<K: AsRef<str>>(&self, key: K) -> bool {
        MapType::contains_key(self, key.as_ref().into())
    }
}

#[cfg(test)]
mod tests {
    use crate::doc::MapType;
    use crate::{Any, Content, Doc};
    use std::sync::Arc;

    #[test]
    fn test_ymap_basic() {
        let doc = Doc::default();
        let mut map = doc.get_or_create_map("map").unwrap();
        map.insert("1", "value").unwrap();
        assert_eq!(
            map.get("1"),
            Some(&Content::Any(vec![Any::String("value".to_string())]))
        );
        assert!(map.contains_key("1"));
        map.remove("1");
        assert!(!map.contains_key("1"));
    }

    #[test]
    fn test_ymap_equal() {
        let doc = Doc::default();
        let mut map = doc.get_or_create_map("map").unwrap();
        map.insert("1", "value").unwrap();
        map.insert("2", false).unwrap();

        let binary = doc.encode_update_v1().unwrap();
        let new_doc = Doc::new_from_binary(binary).unwrap();
        let map = new_doc.get_or_create_map("map").unwrap();
        assert_eq!(
            map.get("1"),
            Some(&Content::Any(vec![Any::String("value".to_string())]))
        );
        assert_eq!(map.get("2"), Some(&Content::Any(vec![Any::False])));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_ymap_iter() {
        let doc = Doc::default();
        let mut map = doc.get_or_create_map("map").unwrap();
        map.insert("1", "value1").unwrap();
        map.insert("2", "value2").unwrap();
        let iter = map.iter();
        assert_eq!(iter.count(), 2);

        let iter = map.iter();
        let mut vec: Vec<_> = iter.collect();
        vec.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            vec[0].content,
            Arc::new(Content::Any(vec![Any::String("value1".to_string())]))
        );
        assert_eq!(
            vec[1].content,
            Arc::new(Content::Any(vec![Any::String("value2".to_string())]))
        );
    }
}
