use crate::{doc::StructInfo, impl_type, Content, JwstCodecResult};

use super::list::ListType;

impl_type!(Text);

impl ListType for Text {
    fn len(&self) -> u64 {
        self.read().content_len
    }
}

impl Text {
    pub fn len(&self) -> u64 {
        self.read().content_len
    }

    pub fn insert<T: ToString>(&mut self, char_index: u64, str: T) -> JwstCodecResult {
        self.insert_content_at(char_index, Content::String(str.to_string()))
    }
}

impl ToString for Text {
    fn to_string(&self) -> String {
        let inner = self.read();

        let mut start = inner.start.clone();
        let mut ret = String::with_capacity(inner.content_len as usize);

        while let Some(cur) = start.take() {
            match cur {
                StructInfo::Item(item) => {
                    if !item.deleted() {
                        if let Content::String(str) = item.content.as_ref() {
                            ret.push_str(str);
                        }
                        start = item.right_id.and_then(|right_id| {
                            inner
                                .store
                                .upgrade()
                                .unwrap()
                                .read()
                                .unwrap()
                                .get_item(right_id)
                        });
                    }
                }
                _ => break,
            }
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use crate::Doc;
    use rand::Rng;
    use yrs::{Text, Transact};

    #[test]
    fn test_manipulate_text() {
        let doc = Doc::default();
        let mut text = doc.create_text().unwrap();

        text.insert(0, "llo").unwrap();
        text.insert(0, "he").unwrap();
        text.insert(5, " world").unwrap();
        text.insert(6, "great ").unwrap();
        text.insert(17, '!').unwrap();

        assert_eq!(text.to_string(), "hello great world!");
    }

    #[test]
    fn test_parallel_manipulate_text() {
        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();
        let mut handles = Vec::new();
        let iteration = 10;

        // parallel editing text
        {
            for i in 0..iteration {
                let mut text = text.clone();
                handles.push(std::thread::spawn(move || {
                    let pos = rand::thread_rng().gen_range(0..text.len());
                    text.insert(pos, format!("hello {i}")).unwrap();
                }));
            }
        }

        // parallel editing doc
        {
            for i in 0..iteration {
                let doc = doc.clone();
                handles.push(std::thread::spawn(move || {
                    let mut text = doc.get_or_crate_text("test").unwrap();
                    let pos = rand::thread_rng().gen_range(0..text.len());
                    text.insert(pos, format!("hello doc{i}")).unwrap();
                }));
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            text.to_string().len(),
            32 /* raw length */
            + 7 * iteration /* parallel text editing: insert(pos, "hello {i}") */
            + 10 * iteration /* parallel doc editing: insert(pos, "hello doc{i}") */
        );
    }

    #[test]
    fn test_recover_from_ybinary() {
        let binary = {
            let doc = yrs::Doc::new();
            let text = doc.get_or_insert_text("greating");
            let mut trx = doc.transact_mut();
            text.insert(&mut trx, 0, "hello").unwrap();
            text.insert(&mut trx, 5, " world!").unwrap();
            text.remove_range(&mut trx, 11, 1).unwrap();

            trx.encode_update_v1().unwrap()
        };

        let doc = Doc::new_from_binary(binary).unwrap();
        let mut text = doc.get_or_crate_text("greating").unwrap();

        assert_eq!(text.len(), 11);

        text.insert(6, "great ").unwrap();
        text.insert(17, '!').unwrap();
        assert_eq!(text.to_string(), "hello great world!");
    }
}
