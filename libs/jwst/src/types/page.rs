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
