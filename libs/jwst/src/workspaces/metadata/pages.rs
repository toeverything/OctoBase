use std::collections::HashMap;

use lib0::any::Any;
use yrs::{types::Value, Array, ArrayRef, Map, MapRef, ReadTxn};

pub struct PageMeta {
    pub id: String,
    pub favorite: Option<bool>,
    pub is_pinboard: Option<bool>,
    pub is_shared: Option<bool>,
    pub init: Option<bool>,
    pub sub_page_ids: Vec<String>,
    pub title: Option<String>,
    pub trash: Option<bool>,
    pub trash_date: Option<usize>,
}

impl PageMeta {
    fn get_bool<T: ReadTxn>(trx: &T, map: &MapRef, key: &str) -> Option<bool> {
        map.get(trx, key).and_then(|v| {
            if let Value::Any(Any::Bool(b)) = v {
                Some(b)
            } else {
                None
            }
        })
    }

    fn get_number<T: ReadTxn>(trx: &T, map: &MapRef, key: &str) -> Option<f64> {
        map.get(trx, key).and_then(|v| {
            if let Value::Any(Any::Number(n)) = v {
                Some(n)
            } else {
                None
            }
        })
    }
}

impl<T: ReadTxn> From<(&T, MapRef)> for PageMeta {
    fn from((trx, map): (&T, MapRef)) -> Self {
        Self {
            id: map.get(trx, "id").unwrap().to_string(trx),
            favorite: Self::get_bool(trx, &map, "favorite"),
            is_pinboard: Self::get_bool(trx, &map, "isRootPinboard"),
            is_shared: Self::get_bool(trx, &map, "isPublic"),
            init: Self::get_bool(trx, &map, "init"),
            sub_page_ids: map
                .get(trx, "subpageIds")
                .and_then(|v| v.to_yarray())
                .map(|v| v.iter(trx).map(|v| v.to_string(trx)).collect::<Vec<_>>())
                .unwrap_or_default(),
            title: map.get(trx, "title").map(|s| s.to_string(trx)),
            trash: Self::get_bool(trx, &map, "trash"),
            trash_date: Self::get_number(trx, &map, "trashDate")
                .map(|v| v as usize)
                .filter(|v| *v > 0),
        }
    }
}

#[derive(Clone)]
pub struct Pages {
    pages: ArrayRef,
}

impl Pages {
    pub fn new(pages: ArrayRef) -> Self {
        Self { pages }
    }

    fn pages<T: ReadTxn>(&self, trx: &T) -> HashMap<String, PageMeta> {
        self.pages
            .iter(trx)
            .filter_map(|v| {
                v.to_ymap().map(|v| {
                    let meta = PageMeta::from((trx, v));
                    (meta.id.clone(), meta)
                })
            })
            .collect()
    }

    fn check_pinboard(pages: &HashMap<String, PageMeta>, page_id: &str) -> bool {
        if let Some(root_pinboard_page) = pages
            .values()
            .find(|meta| meta.is_pinboard.unwrap_or(false))
        {
            let mut visited = vec![];
            let mut stack = vec![root_pinboard_page.id.clone()];
            while let Some(current_page_id) = stack.pop() {
                if visited.contains(&current_page_id) {
                    continue;
                }
                visited.push(current_page_id.clone());
                if let Some(page) = pages.get(&current_page_id) {
                    if page.id == page_id {
                        return true;
                    }
                    stack.extend(page.sub_page_ids.clone());
                }
            }
        }

        false
    }

    pub fn check_shared<T: ReadTxn>(&self, trx: &T, page_id: &str) -> bool {
        let pages = self.pages(trx);
        if pages.contains_key(page_id) {
            Self::check_pinboard(&pages, page_id)
                || pages
                    .values()
                    .any(|meta| meta.is_shared.unwrap_or(false) && meta.id == page_id)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{ArrayPrelim, Doc, Transact};

    #[test]
    fn test_page_meta() {
        let doc = Doc::new();
        let map = doc.get_or_insert_map("test");
        let mut trx = doc.transact_mut();
        map.insert(&mut trx, "id", "test_page").unwrap();
        map.insert(&mut trx, "favorite", true).unwrap();
        map.insert(&mut trx, "isRootPinboard", true).unwrap();
        map.insert(&mut trx, "init", true).unwrap();
        map.insert(&mut trx, "subpageIds", ArrayPrelim::default())
            .unwrap();
        map.insert(&mut trx, "title", "test_title").unwrap();
        map.insert(&mut trx, "trash", true).unwrap();
        map.insert(&mut trx, "trashDate", 1234567890).unwrap();

        let meta = PageMeta::from((&trx, map));
        assert_eq!(meta.id, "test_page");
        assert_eq!(meta.favorite, Some(true));
        assert_eq!(meta.is_pinboard, Some(true));
        assert_eq!(meta.init, Some(true));
        assert_eq!(meta.sub_page_ids, Vec::<String>::new());
        assert_eq!(meta.title, Some("test_title".to_string()));
        assert_eq!(meta.trash, Some(true));
        assert_eq!(meta.trash_date, Some(1234567890));
    }
}
