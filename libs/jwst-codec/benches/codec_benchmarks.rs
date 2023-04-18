use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use jwst_codec::{read_var_i64, read_var_u64, write_var_i64, write_var_u64};
use lib0::decoding::{Cursor, Read};
use lib0::encoding::Write;

const BENCHMARK_SIZE: u32 = 100000;

fn lib0_encoding(c: &mut Criterion) {
    let mut lib0_group = c.benchmark_group("lib0");
    lib0_group.sampling_mode(SamplingMode::Flat);
    lib0_group.bench_function("var_int (64 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as i64) {
                encoder.write_var(i);
            }
            let mut decoder = Cursor::from(&encoder);
            for i in 0..(BENCHMARK_SIZE as i64) {
                let num: i64 = decoder.read_var().unwrap();
                assert_eq!(num, i);
            }
        })
    });
    lib0_group.bench_function("var_uint (32 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..BENCHMARK_SIZE {
                encoder.write_var(i);
            }
            let mut decoder = Cursor::from(&encoder);
            for i in 0..BENCHMARK_SIZE {
                let num: u32 = decoder.read_var().unwrap();
                assert_eq!(num, i);
            }
        })
    });
    lib0_group.bench_function("uint32", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..BENCHMARK_SIZE {
                encoder.write_u32(i);
            }
            let mut decoder = Cursor::from(&encoder);
            for i in 0..BENCHMARK_SIZE {
                let num: u32 = decoder.read_u32().unwrap();
                assert_eq!(num, i);
            }
        })
    });
    lib0_group.bench_function("var_uint (64 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as u64) {
                encoder.write_var(i);
            }
            let mut decoder = Cursor::from(&encoder);
            for i in 0..(BENCHMARK_SIZE as u64) {
                let num: u64 = decoder.read_var().unwrap();
                assert_eq!(num, i);
            }
        })
    });
    lib0_group.bench_function("uint64", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as u64) {
                encoder.write_u64(i)
            }
            let mut decoder = Cursor::from(&encoder);
            for i in 0..(BENCHMARK_SIZE as u64) {
                let num = decoder.read_u64().unwrap();
                assert_eq!(num, i);
            }
        })
    });
}

fn jwst_encoding(c: &mut Criterion) {
    let mut jwst_group = c.benchmark_group("jwst");
    jwst_group.sampling_mode(SamplingMode::Flat);
    jwst_group.bench_function("var_int (64 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);

            for i in 0..(BENCHMARK_SIZE as i64) {
                write_var_i64(&mut encoder, i).unwrap();
            }
            let mut decoder = encoder.as_slice();
            for i in 0..(BENCHMARK_SIZE as i64) {
                let (tail, num) = read_var_i64(decoder).unwrap();
                decoder = tail;
                assert_eq!(num, i);
            }
        })
    });
    jwst_group.bench_function("var_int (32 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);

            for i in 0..(BENCHMARK_SIZE as i32) {
                write_var_i64(&mut encoder, i as i64).unwrap();
            }
            let mut decoder = encoder.as_slice();
            for i in 0..(BENCHMARK_SIZE as i32) {
                let (tail, num) = read_var_i64(decoder).unwrap();
                decoder = tail;
                assert_eq!(num as i32, i);
            }
        })
    });
    jwst_group.bench_function("var_uint (32 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..BENCHMARK_SIZE {
                write_var_u64(&mut encoder, i as u64).unwrap();
            }
            let mut decoder = encoder.as_slice();
            for i in 0..BENCHMARK_SIZE {
                let (tail, num) = read_var_u64(decoder).unwrap();
                decoder = tail;
                assert_eq!(num as u32, i);
            }
        })
    });
    jwst_group.bench_function("var_uint (64 bit)", |b| {
        b.iter(|| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as u64) {
                write_var_u64(&mut encoder, i).unwrap();
            }
            let mut decoder = encoder.as_slice();
            for i in 0..(BENCHMARK_SIZE as u64) {
                let (tail, num) = read_var_u64(decoder).unwrap();
                decoder = tail;
                assert_eq!(num, i);
            }
        })
    });
}

criterion_group!(benches, lib0_encoding, jwst_encoding);
criterion_main!(benches);
