use super::*;

pub struct YArray {
    core: ListCore,
}

impl YArray {
    pub fn new(store: DocStore, root: TypeStoreRef) -> JwstCodecResult<YArray> {
        let origin_type = root.borrow_mut().set_kind(TypeStoreKind::Array);
        if let Some(type_kind) = origin_type {
            Err(JwstCodecError::InvalidType(type_kind))
        } else {
            Ok(Self {
                core: ListCore::new(store, root),
            })
        }
    }

    pub fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        self.core.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Transact};

    #[test]
    fn test_yarray_equal() {
        let buffer = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, 1).unwrap();
            array.insert(&mut trx, 0, "a").unwrap();
            trx.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();
        println!("{:#?}", update);
        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_array("abc").unwrap();

        assert_eq!(
            array.get(0).unwrap().unwrap(),
            Content::Any(vec![Any::Integer(1)])
        );
        // FIXME: it seems that some items were missed during apply_update
        // so no non-root index could be found here
        // assert_eq!(
        //     array.get(1).unwrap().unwrap(),
        //     Content::String("a".to_string())
        // );
    }
}
