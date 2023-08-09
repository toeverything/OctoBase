use jwst_codec::{Any, Array, Map, Value};
use std::collections::HashMap;

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
    fn get_string_array(map: &Map, key: &str) -> Vec<String> {
        map.get(key)
            .and_then(|v| {
                if let Value::Any(Any::Array(a)) = v {
                    Some(
                        a.iter()
                            .filter_map(|sub_page| {
                                if let Any::String(str) = sub_page {
                                    Some(str.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    fn get_bool(map: &Map, key: &str) -> Option<bool> {
        map.get(key).and_then(|v| {
            if let Value::Any(any) = v {
                match any {
                    Any::True => Some(true),
                    Any::False => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn get_number(map: &Map, key: &str) -> Option<f64> {
        map.get(key).and_then(|v| {
            if let Value::Any(Any::Float64(n)) = v {
                Some(n.0)
            } else {
                None
            }
        })
    }
}

impl From<Map> for PageMeta {
    fn from(map: Map) -> Self {
        Self {
            id: map
                .get("id")
                .and_then(|v| v.to_text())
                .map(|t| t.to_string())
                .unwrap_or_default(),
            favorite: Self::get_bool(&map, "favorite"),
            is_pinboard: Self::get_bool(&map, "isRootPinboard"),
            is_shared: Self::get_bool(&map, "isPublic").or_else(|| {
                Self::get_number(&map, "isPublic").map(|exp| {
                    // if isPublic is a number, it is a expire time timestamp
                    let exp = exp as i64;
                    let now = chrono::Utc::now().timestamp();
                    exp > now
                })
            }),
            init: Self::get_bool(&map, "init"),
            sub_page_ids: Self::get_string_array(&map, "subpageIds"),
            title: map
                .get("title")
                .and_then(|s| s.to_text().map(|t| t.to_string())),
            trash: Self::get_bool(&map, "trash"),
            trash_date: Self::get_number(&map, "trashDate")
                .map(|v| v as usize)
                .filter(|v| *v > 0),
        }
    }
}

#[derive(Clone)]
pub struct Pages {
    pages: Array,
}

impl Pages {
    pub fn new(pages: Array) -> Self {
        Self { pages }
    }

    fn pages(&self) -> HashMap<String, PageMeta> {
        self.pages
            .iter()
            .filter_map(|v| {
                v.to_map().map(|v| {
                    let meta = PageMeta::from(v);
                    (meta.id.clone(), meta)
                })
            })
            .collect()
    }

    fn check_pinboard(pages: &HashMap<String, PageMeta>, page_id: &str) -> bool {
        if let Some(root_pinboard_page) = pages
            .values()
            .find(|meta| meta.is_pinboard.unwrap_or(false) && meta.is_shared.unwrap_or(false))
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

    pub fn check_shared(&self, page_id: &str) -> bool {
        let pages = self.pages();
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::Workspace;
//     use jwst_codec::{Doc, Update};

//     #[test]
//     fn test_page_meta() {
//         let doc = Doc::default();
//         let mut map = doc.get_or_create_map("test").unwrap();
//         map.insert(&mut "id", "test_page").unwrap();
//         map.insert(&mut "favorite", true).unwrap();
//         map.insert(&mut "isRootPinboard", true).unwrap();
//         map.insert(&mut "init", true).unwrap();
//         map.insert(&mut "subpageIds", doc.create_array().unwrap())
//             .unwrap();
//         map.insert(&mut "title", "test_title").unwrap();
//         map.insert(&mut "trash", true).unwrap();
//         map.insert(&mut "trashDate", 1234567890).unwrap();

//         let meta = PageMeta::from(&map);
//         assert_eq!(meta.id, "test_page");
//         assert_eq!(meta.favorite, Some(true));
//         assert_eq!(meta.is_pinboard, Some(true));
//         assert_eq!(meta.init, Some(true));
//         assert_eq!(meta.sub_page_ids, Vec::<String>::new());
//         assert_eq!(meta.title, Some("test_title".to_string()));
//         assert_eq!(meta.trash, Some(true));
//         assert_eq!(meta.trash_date, Some(1234567890));
//     }

//     #[test]
//     fn test_shared_page() {
//         let doc = Doc::default();
//         doc.apply_update(
//             Update::from_ybinary1(
//                 include_bytes!("../../../fixtures/test_shared_page.bin").to_vec(),
//             )
//             .unwrap(),
//         );
//         let ws = Workspace::from_doc(doc, "test").unwrap();

//         // - test page (shared page not in Pinboard)
//         assert!(ws.get_space("X83xzrb4Yr").shared());
//         // - test page (unshared sub page of X83xzrb4Yr )
//         assert!(!ws.get_space("ZISRn1STfy").shared());

//         // - test page (RootPinboard without shared)
//         assert!(!ws.get_space("m92E0qWwPY").shared());
//         // - test page (unshared sub page of m92E0qWwPY in Pinboard)
//         assert!(!ws.get_space("2HadvFQVk3").shared());
//         // - test page (shared sub page of 2HadvFQVk3 in Pinboard)
//         assert!(ws.get_space("ymMTOFx8tt").shared());
//         // - test page (unshared sub page of ymMTOFx8tt in Pinboard)
//         assert!(!ws.get_space("lBaYQm5ZVo").shared());
//     }
// }
