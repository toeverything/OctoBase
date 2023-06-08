use crate::{impl_type, Content, JwstCodecResult};

use super::list::ListType;

impl_type!(Text);

impl ListType for Text {}

impl Text {
    #[inline]
    pub fn len(&self) -> u64 {
        self.content_len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn insert<T: ToString>(&mut self, char_index: u64, str: T) -> JwstCodecResult {
        self.insert_at(char_index, vec![Content::String(str.to_string())])
    }

    #[inline]
    pub fn remove(&mut self, char_index: u64, len: u64) -> JwstCodecResult {
        self.remove_at(char_index, len)
    }
}

impl ToString for Text {
    fn to_string(&self) -> String {
        let mut ret = String::with_capacity(self.len() as usize);

        self.iter_item().fold(&mut ret, |ret, item| {
            if let Content::String(str) = item.content.as_ref() {
                ret.push_str(str);
            }

            ret
        });

        ret
    }
}

#[cfg(test)]
mod tests {
    use crate::Doc;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use std::sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    };
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
        assert_eq!(text.len(), 18);

        text.remove(4, 4).unwrap();
        assert_eq!(text.to_string(), "helleat world!");
        assert_eq!(text.len(), 14);
    }

    #[test]
    #[ignore = "block by faster left/right refactoring"]
    fn test_parallel_insert_text() {
        let iteration = 10;
        let rand = ChaCha20Rng::seed_from_u64(rand::thread_rng().gen());
        let added_len = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();

        // parallel editing text
        {
            for i in 0..iteration {
                let mut text = text.clone();
                let mut rand = rand.clone();
                let len = added_len.clone();
                handles.push(std::thread::spawn(move || {
                    let pos = rand.gen_range(0..text.len());
                    let string = format!("hello {i}");

                    text.insert(pos, &string).unwrap();
                    len.fetch_add(string.len(), Ordering::SeqCst);
                }));
            }
        }

        // parallel editing doc
        {
            for i in 0..iteration {
                let doc = doc.clone();
                let mut rand = rand.clone();
                let len = added_len.clone();
                handles.push(std::thread::spawn(move || {
                    let mut text = doc.get_or_crate_text("test").unwrap();
                    let pos = rand.gen_range(0..text.len());
                    let string = format!("hello doc{i}");

                    text.insert(pos, &string).unwrap();
                    len.fetch_add(string.len(), Ordering::SeqCst);
                }));
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            text.to_string().len(),
            32 /* raw length */
            + added_len.load(Ordering::SeqCst) /* parallel text editing: insert(pos, string) */
        );
    }

    #[test]
    fn test_parallel_ins_del_text() {
        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();

        let mut handles = Vec::new();
        let iteration = 2;
        let len = Arc::new(AtomicU64::new(32));

        for i in 0..iteration {
            let mut text = text.clone();
            let len = len.clone();
            handles.push(std::thread::spawn(move || {
                let ins_del = rand::random::<bool>();
                if ins_del {
                    let str = format!("hello {i}");
                    let pos = rand::thread_rng().gen_range(0..text.len());
                    text.insert(pos, &str).unwrap();
                    len.fetch_add(str.len() as u64, Ordering::SeqCst);
                } else {
                    let pos = rand::thread_rng().gen_range(0..text.len() / 2);
                    text.remove(pos, 6).unwrap();
                    len.fetch_sub(6, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(text.to_string().len() as u64, len.load(Ordering::SeqCst));
        assert_eq!(text.len(), len.load(Ordering::SeqCst));
    }

    #[test]
    fn test_recover_from_yjs_encoder() {
        let binary = {
            let doc = yrs::Doc::with_client_id(1);
            let text = doc.get_or_insert_text("greating");
            let mut trx = doc.transact_mut();
            text.insert(&mut trx, 0, "hello").unwrap();
            text.insert(&mut trx, 5, " world!").unwrap();
            text.remove_range(&mut trx, 11, 1).unwrap();

            trx.encode_update_v1().unwrap()
        };

        let doc = Doc::new_from_binary(binary).unwrap();
        let mut text = doc.get_or_crate_text("greating").unwrap();

        assert_eq!(text.to_string(), "hello world");

        text.insert(6, "great ").unwrap();
        text.insert(17, '!').unwrap();
        assert_eq!(text.to_string(), "hello great world!");
    }

    #[test]
    #[ignore = "encoder output suspicious binary"]
    fn test_recover_from_octobase_encoder() {
        let binary = {
            let doc = Doc::with_client(1);
            let mut text = doc.get_or_crate_text("greating").unwrap();
            text.insert(0, "hello").unwrap();
            text.insert(5, " world!").unwrap();
            text.remove(11, 1).unwrap();

            doc.encode_update_v1().unwrap()
        };

        let doc = Doc::new_from_binary(binary).unwrap();
        let mut text = doc.get_or_crate_text("greating").unwrap();

        assert_eq!(text.to_string(), "hello world");

        text.insert(6, "great ").unwrap();
        text.insert(17, '!').unwrap();
        assert_eq!(text.to_string(), "hello great world!");
    }
}
