use crate::doc::{AsInner, ItemBuilder, Parent, StructInfo, YTypeRef};
use crate::{impl_type, Any, Content, JwstCodecResult};
use std::collections::HashMap;
use std::sync::Arc;

impl_type!(Map);

pub(crate) trait MapType: AsInner<Inner = YTypeRef> {
    fn insert(&mut self, key: String, value: Content) -> JwstCodecResult {
        let mut inner = self.as_inner().write().unwrap();
        let left_id = inner.map.as_ref().and_then(|map| {
            map.get(key.as_str())
                .and_then(|struct_info| struct_info.left_id())
        });
        if let Some(store) = inner.store.upgrade() {
            let mut store = store.write().unwrap();
            let new_item_id = (store.client(), store.get_state(store.client())).into();
            let item = Arc::new(
                ItemBuilder::new()
                    .id(new_item_id)
                    .left_id(left_id)
                    .content(value)
                    .parent(Some(Parent::Type(self.as_inner().clone())))
                    .parent_sub(Some(key))
                    .build(),
            );
            store.integrate_struct_info(StructInfo::Item(item), 0, Some(&mut inner))?;
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
            .map(|struct_info| struct_info.as_item())
            .flatten()
            .map(|item| {
                let ptr = Arc::as_ptr(&item.content);
                unsafe { &*ptr }
            })
    }

    fn get_all(&self) -> HashMap<String, Option<&Content>> {
        let mut ret = HashMap::default();
        let inner = self.as_inner().read().unwrap();
        inner.map.as_ref().map(|map| {
            for key in map.keys() {
                ret.insert(key.clone(), self.get(key.to_string()));
            }
        });

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
        inner
            .map
            .as_ref()
            .and_then(|map| map.get(key.as_str()))
            .map(|struct_info| struct_info.delete());
    }
}

impl MapType for Map {}

impl Map {
    pub fn insert<K: Into<String>, V: Into<Any>>(&mut self, key: K, value: V) -> JwstCodecResult {
        let any = value.into();
        MapType::insert(self, key.into(), any.into())
    }

    pub fn get<K: Into<String>>(&self, key: K) -> Option<&Content> {
        MapType::get(self, key.into())
    }

    pub fn remove<K: Into<String>>(&mut self, key: K) {
        MapType::remove(self, key.into())
    }

    pub fn contains_key<K: Into<String>>(&self, key: K) -> bool {
        MapType::contains_key(self, key.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Any, Content, Doc};

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

        let binary = doc.encode_update_v1().unwrap();
        let new_doc = Doc::new_from_binary(binary).unwrap();
        let map = new_doc.get_or_create_map("map").unwrap();
        assert_eq!(
            map.get("1"),
            Some(&Content::Any(vec![Any::String("value".to_string())]))
        );
    }
}
