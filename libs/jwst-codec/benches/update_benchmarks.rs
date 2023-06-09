mod utils;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use path_ext::PathExt;
use std::time::Duration;
use utils::Files;

fn update(c: &mut Criterion) {
    let files = Files::load();

    let mut group = c.benchmark_group("update");
    group.measurement_time(Duration::from_secs(15));

    for file in &files.files {
        group.throughput(Throughput::Bytes(file.content.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse with jwst", file.path.name_str()),
            &file.content,
            |b, content| {
                b.iter(|| {
                    use jwst_codec::*;
                    let mut decoder = RawDecoder::new(content.clone());
                    Update::read(&mut decoder).unwrap()
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("parse with yrs", file.path.name_str()),
            &file.content,
            |b, content| {
                b.iter(|| {
                    use yrs::{updates::decoder::Decode, Update};
                    Update::decode_v1(&content).unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, update);
criterion_main!(benches);
