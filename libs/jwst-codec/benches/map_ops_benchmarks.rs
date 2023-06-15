use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ops/map");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("jwst/insert", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();

        b.iter(|| {
            use jwst_codec::*;
            let doc = Doc::default();
            let mut map = doc.get_or_create_map("test").unwrap();
            for (idx, key) in base_text.iter().enumerate() {
                map.insert(key, idx).unwrap();
            }
        });
    });

    group.bench_function("yrs/insert", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();

        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let map = doc.get_or_insert_map("test");

            let mut trx = doc.transact_mut();
            for (idx, key) in base_text.iter().enumerate() {
                map.insert(&mut trx, key.to_string(), idx as f64).unwrap();
            }

            drop(trx);
        });
    });

    group.bench_function("jwst/get", |b| {
        use jwst_codec::*;

        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();
        let doc = Doc::default();
        let mut map = doc.get_or_create_map("test").unwrap();
        for (idx, key) in base_text.iter().enumerate() {
            map.insert(key, idx).unwrap();
        }

        b.iter(|| {
            for key in &base_text {
                map.get(key);
            }
        });
    });

    group.bench_function("yrs/get", |b| {
        use yrs::*;

        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();

        let doc = Doc::new();
        let map = doc.get_or_insert_map("test");

        let mut trx = doc.transact_mut();
        for (idx, key) in (&base_text).iter().enumerate() {
            map.insert(&mut trx, key.to_string(), idx as f64).unwrap();
        }
        drop(trx);

        b.iter(|| {
            let trx = doc.transact();
            for key in &base_text {
                map.get(&trx, key).unwrap();
            }
        });
    });

    group.bench_function("jwst/remove", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();

        b.iter(|| {
            use jwst_codec::*;
            let doc = Doc::default();
            let mut map = doc.get_or_create_map("test").unwrap();
            for (idx, key) in base_text.iter().enumerate() {
                map.insert(key, idx).unwrap();
            }
            for key in &base_text {
                map.remove(key);
            }
        });
    });

    group.bench_function("yrs/remove", |b| {
        let base_text = "test1 test2 test3 test4 test5 test6 test7 test8 test9"
            .split(" ")
            .collect::<Vec<_>>();

        b.iter(|| {
            use yrs::*;
            let doc = Doc::new();
            let map = doc.get_or_insert_map("test");

            let mut trx = doc.transact_mut();
            for (idx, key) in (&base_text).iter().enumerate() {
                map.insert(&mut trx, key.to_string(), idx as f64).unwrap();
            }

            for key in &base_text {
                map.remove(&mut trx, key).unwrap();
            }

            drop(trx);
        });
    });

    group.finish();
}

criterion_group!(benches, operations);
criterion_main!(benches);
