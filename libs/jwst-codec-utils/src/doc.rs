#[cfg(test)]
mod tests {
    use jwst_codec::Doc;
    use yrs::{Map, Transact};

    #[test]
    fn test_basic_yrs_binary_compatibility() {
        let yrs_doc = yrs::Doc::new();

        let map = yrs_doc.get_or_insert_map("abc");
        let mut trx = yrs_doc.transact_mut();
        map.insert(&mut trx, "a", 1).unwrap();

        let binary_from_yrs = trx.encode_update_v1().unwrap();

        let doc = Doc::new_from_binary(binary_from_yrs.clone()).unwrap();
        let binary = doc.encode_update_v1().unwrap();

        assert_eq!(binary_from_yrs, binary);
    }
}
