use super::*;

pub struct YArray {
    core: ListCore,
}

impl YArray {
    pub fn new(store: DocStore, root: TypeStoreRef) -> JwstCodecResult<YArray> {
        let origin_type = root.borrow_mut().set_kind(TypeStoreKind::Array);
        if let Some(type_kind) = origin_type {
            Err(JwstCodecError::InvalidInitType(type_kind))
        } else {
            Ok(Self {
                core: ListCore::new(store, root),
            })
        }
    }

    pub fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        self.core.get(index)
    }

    pub fn iter(&self) -> ListIterator {
        self.core.iter()
    }

    pub fn map<F, T>(&self, f: F) -> impl Iterator<Item = JwstCodecResult<T>>
    where
        F: FnMut(Content) -> JwstCodecResult<T>,
    {
        self.core.map(f)
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
        let array = doc.get_array("abc").unwrap();

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
            let array = doc.get_array("abc").unwrap();

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
            let array = doc.get_array("abc").unwrap();

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
}
