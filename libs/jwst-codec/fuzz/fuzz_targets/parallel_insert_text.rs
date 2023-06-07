#![no_main]

use jwst_codec::*;
use libfuzzer_sys::fuzz_target;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

fuzz_target!(|seed: u64| {
    println!("seed: {}", seed);
    let iteration = 10;
    let rand = ChaCha20Rng::seed_from_u64(seed);

    let doc = Doc::default();
    let mut text = doc.get_or_crate_text("test").unwrap();
    text.insert(0, "This is a string with length 32.").unwrap();

    for i in 0..iteration {
        let mut text = text.clone();
        let mut rand = rand.clone();
        let pos = rand.gen_range(0..text.len());
        text.insert(pos, format!("hello {i}")).unwrap();
    }

    for i in 0..iteration {
        let doc = doc.clone();
        let mut rand = rand.clone();
        let mut text = doc.get_or_crate_text("test").unwrap();
        let pos = rand.gen_range(0..text.len());
        text.insert(pos, format!("hello doc{i}")).unwrap();
    }

    assert_eq!(
        text.to_string().len(),
        32 /* raw length */
        + 7 * iteration /* parallel text editing: insert(pos, "hello {i}") */
        + 10 * iteration /* parallel doc editing: insert(pos, "hello doc{i}") */
    );
});
