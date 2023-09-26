use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng};

fn operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ops/text");
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
            let text = doc.get_or_insert_text("test");
            let mut trx = doc.transact_mut();

            text.push(&mut trx, base_text).unwrap();
            for idx in &idxs {
                text.insert(&mut trx, *idx, "test").unwrap();
            }
            drop(trx);
        });
    });

    group.bench_function("yrs/remove", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9";

        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let text = doc.get_or_insert_text("test");
            let mut trx = doc.transact_mut();

            text.push(&mut trx, base_text).unwrap();
            text.push(&mut trx, base_text).unwrap();
            text.push(&mut trx, base_text).unwrap();
            for idx in (base_text.len() as u32)..0 {
                text.remove_range(&mut trx, idx, 1).unwrap();
            }
            drop(trx);
        });
    });

    group.finish();
}

criterion_group!(benches, operations);
criterion_main!(benches);
