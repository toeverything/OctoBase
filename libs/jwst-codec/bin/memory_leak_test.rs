use jwst_codec::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

fn run_text_test(seed: u64) {
    let doc = Doc::with_client(1);
    let mut rand = ChaCha20Rng::seed_from_u64(seed);
    let mut text = doc.get_or_create_text("test").unwrap();
    text.insert(0, "This is a string with length 32.").unwrap();

    let iteration = 20;
    let mut len = 32;

    for i in 0..iteration {
        let mut text = text.clone();
        let ins = i % 2 == 0;
        let pos = rand.gen_range(0..if ins { text.len() } else { len / 2 });
        if ins {
            let str = format!("hello {i}");
            text.insert(pos, &str).unwrap();
            len += str.len() as u64;
        } else {
            text.remove(pos, 6).unwrap();
            len -= 6;
        }
    }

    assert_eq!(text.to_string().len(), len as usize);
    assert_eq!(text.len(), len);
}

fn run_array_test(seed: u64) {
    let doc = Doc::with_client(1);
    let mut rand = ChaCha20Rng::seed_from_u64(seed);
    let mut array = doc.get_or_create_array("test").unwrap();
    array.push(1).unwrap();

    let iteration = 20;
    let mut len = 1;

    for i in 0..iteration {
        let mut array = array.clone();
        let ins = i % 2 == 0;
        let pos = rand.gen_range(0..if ins { array.len() } else { len / 2 });
        if ins {
            array.insert(pos, 1).unwrap();
            len += 1;
        } else {
            array.remove(pos, 1).unwrap();
            len -= 1;
        }
    }

    assert_eq!(array.len(), len);
}

fn main() {
    let mut rand = ChaCha20Rng::seed_from_u64(rand::thread_rng().gen());
    for _ in 0..10000 {
        let seed = rand.gen();
        run_array_test(seed);
        run_text_test(seed);
    }
}
