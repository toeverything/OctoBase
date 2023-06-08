#![no_main]

use jwst_codec::*;
use libfuzzer_sys::fuzz_target;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fuzz_target!(|seed: u64| {
    println!("seed: {}", seed);
    let iteration = 10;
    let rand = ChaCha20Rng::seed_from_u64(seed);
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
});
