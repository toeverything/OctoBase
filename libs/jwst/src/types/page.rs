use lib0::any::Any;
use yrs::{types::Value, Array, Map, MapRef, ReadTxn};

pub struct PageMeta {
    pub id: String,
    pub favorite: Option<bool>,
    pub is_pivots: Option<bool>,
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
            is_pivots: Self::get_bool(trx, &map, "is_pivots"),
            init: Self::get_bool(trx, &map, "init"),
            sub_page_ids: map
                .get(trx, "sub_page_ids")
                .and_then(|v| v.to_yarray())
                .map(|v| v.iter(trx).map(|v| v.to_string(trx)).collect::<Vec<_>>())
                .unwrap_or_default(),
            title: map.get(trx, "title").map(|s| s.to_string(trx)),
            trash: Self::get_bool(trx, &map, "trash"),
            trash_date: Self::get_number(trx, &map, "trash_date")
                .map(|v| v as usize)
                .filter(|v| *v > 0),
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
        map.insert(&mut trx, "is_pivots", true).unwrap();
        map.insert(&mut trx, "init", true).unwrap();
        map.insert(&mut trx, "sub_page_ids", ArrayPrelim::default())
            .unwrap();
        map.insert(&mut trx, "title", "test_title").unwrap();
        map.insert(&mut trx, "trash", true).unwrap();
        map.insert(&mut trx, "trash_date", 1234567890).unwrap();

        let meta = PageMeta::from((&trx, map));
        assert_eq!(meta.id, "test_page");
        assert_eq!(meta.favorite, Some(true));
        assert_eq!(meta.is_pivots, Some(true));
        assert_eq!(meta.init, Some(true));
        assert_eq!(meta.sub_page_ids, Vec::<String>::new());
        assert_eq!(meta.title, Some("test_title".to_string()));
        assert_eq!(meta.trash, Some(true));
        assert_eq!(meta.trash_date, Some(1234567890));
    }
}
