use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng};

fn operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ops/array");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("yrs/insert", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9";
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(1234);

        let idxs = (0..99)
            .map(|_| rng.gen_range(0..base_text.len() as u32))
            .collect::<Vec<_>>();
        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let array = doc.get_or_insert_array("test");

            let mut trx = doc.transact_mut();
            for c in base_text.chars() {
                array.push_back(&mut trx, c.to_string()).unwrap();
            }
            for idx in &idxs {
                array.insert(&mut trx, *idx, "test").unwrap();
            }
            drop(trx);
        });
    });

    group.bench_function("yrs/insert range", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9";
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(1234);

        let idxs = (0..99)
            .map(|_| rng.gen_range(0..base_text.len() as u32))
            .collect::<Vec<_>>();
        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let array = doc.get_or_insert_array("test");

            let mut trx = doc.transact_mut();
            for c in base_text.chars() {
                array.push_back(&mut trx, c.to_string()).unwrap();
            }
            for idx in &idxs {
                array.insert_range(&mut trx, *idx, vec!["test1", "test2"]).unwrap();
            }
            drop(trx);
        });
    });

    group.bench_function("yrs/remove", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9";

        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let array = doc.get_or_insert_array("test");

            let mut trx = doc.transact_mut();
            for c in base_text.chars() {
                array.push_back(&mut trx, c.to_string()).unwrap();
            }
            for idx in (base_text.len() as u32)..0 {
                array.remove(&mut trx, idx).unwrap();
            }
            drop(trx);
        });
    });

    group.finish();
}

criterion_group!(benches, operations);
criterion_main!(benches);
