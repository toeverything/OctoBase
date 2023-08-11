use super::*;

impl_type!(Array);

impl ListType for Array {}

pub struct ArrayIter(ListIterator);

impl Iterator for ArrayIter {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        for item in self.0.by_ref() {
            if let Some(item) = item.get() {
                if item.countable() {
                    return Some(item.content.as_ref().try_into().unwrap());
                }
            }
        }

        None
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

    pub fn get(&self, index: u64) -> Option<Value> {
        let (item, offset) = self.get_item_at(index)?;
        // array only store 1-unit elements
        debug_assert!(offset == 0);
        if let Some(item) = item.get() {
            // TODO: rewrite to content.read(&mut [Any])
            return match item.content.as_ref() {
                Content::Any(any) => return any.first().map(|any| Value::Any(any.clone())),
                _ => item.content.as_ref().try_into().map_or_else(|_| None, Some),
            };
        }

        None
    }

    pub fn iter(&self) -> ArrayIter {
        ArrayIter(self.iter_item())
    }

    pub fn push<V: Into<Value>>(&mut self, val: V) -> JwstCodecResult {
        self.insert(self.len(), val)
    }

    pub fn insert<V: Into<Value>>(&mut self, idx: u64, val: V) -> JwstCodecResult {
        self.insert_at(idx, val.into().into())
    }

    pub fn remove(&mut self, idx: u64, len: u64) -> JwstCodecResult {
        self.remove_at(idx, len)
    }
}

impl serde::Serialize for Array {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(Some(self.len() as usize))?;

        for item in self.iter() {
            seq.serialize_element(&item)?;
        }
        seq.end()
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
                Value::Any(Any::String("Hello".into()))
            );
            assert_eq!(array.get(1).unwrap(), Value::Any(Any::String(" ".into())));
            assert_eq!(
                array.get(2).unwrap(),
                Value::Any(Any::String("World".into()))
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

            assert_eq!(
                array.get(0).unwrap(),
                Value::Any(Any::String("Hello".into()))
            );
            assert_eq!(array.get(5).unwrap(), Value::Any(Any::String(" ".into())));
            assert_eq!(
                array.get(6).unwrap(),
                Value::Any(Any::String("World".into()))
            );
            assert_eq!(array.get(11).unwrap(), Value::Any(Any::String("!".into())));
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

            assert_eq!(
                array.get(0).unwrap(),
                Value::Any(Any::String("Hello".into()))
            );
            assert_eq!(array.get(5).unwrap(), Value::Any(Any::String(" ".into())));
            assert_eq!(
                array.get(6).unwrap(),
                Value::Any(Any::String("World".into()))
            );
            assert_eq!(array.get(11).unwrap(), Value::Any(Any::String("!".into())));
        });
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_yrs_array_decode() {
        let update = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");
            let mut trx = doc.transact_mut();

            array.insert(&mut trx, 0, "hello").unwrap();
            array.insert(&mut trx, 1, "world").unwrap();
            array.insert(&mut trx, 1, " ").unwrap();

            trx.encode_update_v1().unwrap()
        };

        loom_model!({
            let doc = Doc::new_from_binary_with_options(
                update.clone(),
                DocOptions {
                    guid: Some(String::from("1")),
                    client: Some(1),
                },
            )
            .unwrap();
            let arr = doc.get_or_create_array("abc").unwrap();

            assert_eq!(
                arr.get(2).unwrap(),
                Value::Any(Any::String("world".to_string()))
            )
        });
    }
}
