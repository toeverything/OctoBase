use std::ops::{Index, Range};

use super::*;

impl_type!(Array);

impl ListType for Array {}

pub struct ArrayIter(ListIterator);

impl Iterator for ArrayIter {
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

    pub fn push<V: Into<Content>>(&mut self, val: V) -> JwstCodecResult {
        self.insert(self.len(), val)
    }

    pub fn insert<V: Into<Content>>(&mut self, idx: u64, val: V) -> JwstCodecResult {
        let content = val.into();
        self.insert_at(idx, content)
    }

    pub fn remove(&mut self, idx: u64, len: u64) -> JwstCodecResult {
        self.remove_at(idx, len)
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

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Options, Text, Transact};

    #[test]
    fn test_yarray_insert() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        loom_model!({
            let doc = Doc::with_options(options.clone());
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
        });
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_ytext_equal() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };
        let yrs_options = Options {
            client_id: rand::random(),
            guid: nanoid::nanoid!().into(),
            ..Default::default()
        };

        loom_model!({
            let doc = yrs::Doc::with_options(yrs_options.clone());
            let array = doc.get_or_insert_text("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 6, "World").unwrap();
            array.insert(&mut trx, 11, "!").unwrap();
            let buffer = trx.encode_update_v1().unwrap();

            let mut decoder = RawDecoder::new(buffer);
            let update = Update::read(&mut decoder).unwrap();

            let mut doc = Doc::with_options(options.clone());
            doc.apply_update(update).unwrap();
            let array = doc.get_or_create_array("abc").unwrap();

            assert_eq!(array.get(0).unwrap(), &Content::String("Hello".into()));
            assert_eq!(array.get(5).unwrap(), &Content::String(" ".into()));
            assert_eq!(array.get(6).unwrap(), &Content::String("World".into()));
            assert_eq!(array.get(11).unwrap(), &Content::String("!".into()));
        });

        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };
        let yrs_options = Options {
            client_id: rand::random(),
            guid: nanoid::nanoid!().into(),
            ..Default::default()
        };
        loom_model!({
            let doc = yrs::Doc::with_options(yrs_options.clone());
            let array = doc.get_or_insert_text("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 5, " ").unwrap();
            array.insert(&mut trx, 6, "World").unwrap();
            array.insert(&mut trx, 11, "!").unwrap();
            let buffer = trx.encode_update_v1().unwrap();

            let mut decoder = RawDecoder::new(buffer);
            let update = Update::read(&mut decoder).unwrap();

            let mut doc = Doc::with_options(options.clone());
            doc.apply_update(update).unwrap();
            let array = doc.get_or_create_array("abc").unwrap();

            assert_eq!(array.get(0).unwrap(), &Content::String("Hello".into()));
            assert_eq!(array.get(5).unwrap(), &Content::String(" ".into()));
            assert_eq!(array.get(6).unwrap(), &Content::String("World".into()));
            assert_eq!(array.get(11).unwrap(), &Content::String("!".into()));
        });
    }

    #[test]
    #[ignore = "TODO"]
    fn test_yarray_slice() {
        loom_model!({
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
        });
    }
}
