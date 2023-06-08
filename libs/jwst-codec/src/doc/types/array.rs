use ordered_float::OrderedFloat;
use std::ops::{Index, Range};

use super::*;

impl_type!(Array);

impl ListType for Array {}

pub struct ArrayIter<'a>(ListIterator<'a>);

impl<'a> Iterator for ArrayIter<'a> {
    type Item = Arc<Content>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|item| item.content.clone())
    }
}

impl Array {
    #[inline]
    pub fn len(&self) -> u64 {
        self.content_len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: u64) -> Option<&Content> {
        self.get_item_at(index).map(|(item, _)| {
            let ptr = Arc::as_ptr(&item.content);

            unsafe { &*ptr }
        })
    }

    pub fn iter(&self) -> ArrayIter {
        ArrayIter(self.iter_item())
    }

    pub fn push<V: Into<Any>>(&mut self, val: V) -> JwstCodecResult {
        self.insert(self.len(), val)
    }

    pub fn insert<V: Into<Any>>(&mut self, idx: u64, val: V) -> JwstCodecResult {
        let contents = Self::group_content(val);
        self.insert_at(idx, contents)
    }

    pub fn remove(&mut self, idx: u64, len: u64) -> JwstCodecResult {
        self.remove_at(idx, len)
    }

    #[inline(always)]
    fn group_content<V: Into<Any>>(val: V) -> Vec<Content> {
        let any = val.into();
        vec![any.into()]
    }
}

impl Index<u64> for Array {
    type Output = Content;

    fn index(&self, index: u64) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl Index<Range<u64>> for Array {
    type Output = [Content];

    fn index(&self, _index: Range<u64>) -> &Self::Output {
        todo!()
    }
}

// TODO: impl for Any::Undefined
// Move to codec/any.rs
impl From<String> for Any {
    fn from(s: String) -> Self {
        Any::String(s)
    }
}

impl From<&str> for Any {
    fn from(s: &str) -> Self {
        Any::from(s.to_string())
    }
}

impl<T: Into<Any>> From<Option<T>> for Any {
    fn from(value: Option<T>) -> Self {
        if let Some(val) = value {
            value.into()
        } else {
            Any::Null
        }
    }
}

impl From<u64> for Any {
    fn from(value: u64) -> Self {
        Any::Integer(value)
    }
}

impl From<OrderedFloat<f32>> for Any {
    fn from(value: OrderedFloat<f32>) -> Self {
        Any::Float32(value)
    }
}

impl From<OrderedFloat<f64>> for Any {
    fn from(value: OrderedFloat<f64>) -> Self {
        Any::Float64(value)
    }
}

impl From<i64> for Any {
    fn from(value: i64) -> Self {
        Any::BigInt64(value)
    }
}

impl From<bool> for Any {
    fn from(value: bool) -> Self {
        if value {
            Any::True
        } else {
            Any::False
        }
    }
}

impl From<HashMap<String, Any>> for Any {
    fn from(value: HashMap<String, Any>) -> Self {
        Any::Object(value)
    }
}

impl From<Vec<Any>> for Any {
    fn from(value: Vec<Any>) -> Self {
        Any::Array(value)
    }
}

impl From<Vec<u8>> for Any {
    fn from(value: Vec<u8>) -> Self {
        Any::Binary(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Text, Transact};

    #[test]
    fn test_yarray_insert() {
        let doc = Doc::default();
        let mut array = doc.get_or_create_array("abc").unwrap();

        array.insert(0, " ").unwrap();
        array.insert(0, "Hello").unwrap();
        array.insert(2, "World").unwrap();

        assert_eq!(
            array.get(0).unwrap(),
            &Content::Any(vec![Any::String("Hello".into())])
        );
        assert_eq!(
            array.get(1).unwrap(),
            &Content::Any(vec![Any::String(" ".into())])
        );
        assert_eq!(
            array.get(2).unwrap(),
            &Content::Any(vec![Any::String("World".into())])
        );
    }

    #[test]
    fn test_ytext_equal() {
        {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_text("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 6, "World").unwrap();
            array.insert(&mut trx, 11, "!").unwrap();
            let buffer = trx.encode_update_v1().unwrap();

            let mut decoder = RawDecoder::new(buffer);
            let update = Update::read(&mut decoder).unwrap();

            let mut doc = Doc::default();
            doc.apply_update(update).unwrap();
            let array = doc.get_or_create_array("abc").unwrap();

            assert_eq!(array.get(0).unwrap(), &Content::String("Hello".into()));
            assert_eq!(array.get(5).unwrap(), &Content::String(" ".into()));
            assert_eq!(array.get(6).unwrap(), &Content::String("World".into()));
            assert_eq!(array.get(11).unwrap(), &Content::String("!".into()));
        }

        {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_text("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 5, " ").unwrap();
            array.insert(&mut trx, 6, "World").unwrap();
            array.insert(&mut trx, 11, "!").unwrap();
            let buffer = trx.encode_update_v1().unwrap();

            let mut decoder = RawDecoder::new(buffer);
            let update = Update::read(&mut decoder).unwrap();

            let mut doc = Doc::default();
            doc.apply_update(update).unwrap();
            let array = doc.get_or_create_array("abc").unwrap();

            assert_eq!(array.get(0).unwrap(), &Content::String("Hello".into()));
            assert_eq!(array.get(5).unwrap(), &Content::String(" ".into()));
            assert_eq!(array.get(6).unwrap(), &Content::String("World".into()));
            assert_eq!(array.get(11).unwrap(), &Content::String("!".into()));
        }
    }

    #[test]
    #[ignore = "TODO"]
    fn test_yarray_slice() {
        let buffer = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, 1).unwrap();
            array.insert(&mut trx, 1, "2").unwrap();
            array.insert(&mut trx, 2, true).unwrap();
            array.insert(&mut trx, 3, 1.0).unwrap();
            trx.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();
        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_or_create_array("abc").unwrap();

        let items = &array[1..3];
        assert_eq!(
            items,
            vec![
                Content::Any(vec![Any::String("2".into())]),
                Content::Any(vec![Any::True])
            ]
        );
    }
}
