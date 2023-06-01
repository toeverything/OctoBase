use super::*;

pub struct YArray {
    core: ListCore,
}

impl YArray {
    pub fn new(client_id: Client, list: Array) -> JwstCodecResult<YArray> {
        Ok(Self {
            core: ListCore::new(client_id, list.inner()),
        })
    }

    pub fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        self.core.get(index)
    }

    pub fn iter(&self) -> ListIterator {
        self.core.iter()
    }

    pub fn map<'a, F, T>(&'a self, f: F) -> impl Iterator<Item = JwstCodecResult<T>> + 'a
    where
        F: FnMut(Content) -> JwstCodecResult<T> + 'a,
    {
        // TODO: the iterator should be short-circuited when it fails
        self.core.iter().flatten().map(f)
    }

    pub fn slice(&self, start: usize, end: usize) -> JwstCodecResult<Vec<Content>> {
        self.iter().skip(start).take(end - start).collect()
    }

    pub fn insert<S: Into<String>>(&mut self, idx: u64, val: S) -> JwstCodecResult {
        self.core.insert(idx, vec![Any::String(val.into())])
    }

    pub fn push<S: Into<String>>(&mut self, val: S) -> JwstCodecResult {
        self.core.push(vec![Any::String(val.into())])
    }

    pub fn remove(&mut self, idx: u64, length: u64) -> JwstCodecResult {
        self.core.remove(idx, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Text, Transact};

    #[test]
    fn test_yarray_equal() {
        let buffer = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 2, "World").unwrap();
            trx.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();

        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_or_crate_array("abc").unwrap();

        assert_eq!(
            array.get(0).unwrap().unwrap(),
            Content::Any(vec![Any::String("Hello".into())])
        );
        assert_eq!(
            array.get(1).unwrap().unwrap(),
            Content::Any(vec![Any::String(" ".into())])
        );
        assert_eq!(
            array.get(2).unwrap().unwrap(),
            Content::Any(vec![Any::String("World".into())])
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
            let array = doc.get_or_crate_array("abc").unwrap();

            assert_eq!(
                (0..=12)
                    .filter_map(|i| array.get(i).ok().flatten().and_then(|c| {
                        if let Content::String(s) = c {
                            Some(s)
                        } else {
                            None
                        }
                    }))
                    .collect::<Vec<_>>()
                    .join(""),
                "Hello World!"
            );
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
            let array = doc.get_or_crate_array("abc").unwrap();

            assert_eq!(
                (0..=12)
                    .filter_map(|i| array.get(i).ok().flatten().and_then(|c| {
                        if let Content::String(s) = c {
                            Some(s)
                        } else {
                            None
                        }
                    }))
                    .collect::<Vec<_>>()
                    .join(""),
                "Hello World!"
            );
        }
    }

    #[test]
    fn test_yarray_map() {
        let buffer = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 2, "World").unwrap();
            array.insert(&mut trx, 3, 1).unwrap();
            trx.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();
        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_or_crate_array("abc").unwrap();

        let items = array
            .iter()
            .filter_map(|c| {
                c.ok()
                    .map(|c| matches!(c, Content::Any(any) if any.len() == 1 && matches!(any[0], Any::String(_))))
            })
            .collect::<Vec<_>>();
        assert_eq!(items, vec![true, true, true, false]);
    }

    #[test]
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
        let array = doc.get_or_crate_array("abc").unwrap();

        let items = array.slice(1, 3).unwrap();
        assert_eq!(
            items,
            vec![
                Content::Any(vec![Any::String("2".into())]),
                Content::Any(vec![Any::True])
            ]
        );
    }
}
