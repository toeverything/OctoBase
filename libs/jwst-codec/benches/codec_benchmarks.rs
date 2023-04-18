use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use jwst_codec::{read_var_i64, read_var_u64, write_var_i64, write_var_u64};
use lib0::decoding::{Cursor, Read};
use lib0::encoding::Write;

const BENCHMARK_SIZE: u32 = 100000;

fn codec(c: &mut Criterion) {
    let mut codec_group = c.benchmark_group("codec");
    codec_group.sampling_mode(SamplingMode::Flat);
    {
        codec_group.bench_function("lib0 encode var_int (64 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..(BENCHMARK_SIZE as i64) {
                    encoder.write_var(i);
                }
            })
        });
        codec_group.bench_function("lib0 decode var_int (64 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as i64) {
                encoder.write_var(i);
            }

            b.iter(|| {
                let mut decoder = Cursor::from(&encoder);
                for i in 0..(BENCHMARK_SIZE as i64) {
                    let num: i64 = decoder.read_var().unwrap();
                    assert_eq!(num, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("lib0 encode var_uint (32 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..BENCHMARK_SIZE {
                    encoder.write_var(i);
                }
            })
        });
        codec_group.bench_function("lib0 decode var_uint (32 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..BENCHMARK_SIZE {
                encoder.write_var(i);
            }

            b.iter(|| {
                let mut decoder = Cursor::from(&encoder);
                for i in 0..BENCHMARK_SIZE {
                    let num: u32 = decoder.read_var().unwrap();
                    assert_eq!(num, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("lib0 encode var_uint (64 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..(BENCHMARK_SIZE as u64) {
                    encoder.write_var(i);
                }
            })
        });
        codec_group.bench_function("lib0 decode var_uint (64 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as u64) {
                encoder.write_var(i);
            }

            b.iter(|| {
                let mut decoder = Cursor::from(&encoder);
                for i in 0..(BENCHMARK_SIZE as u64) {
                    let num: u64 = decoder.read_var().unwrap();
                    assert_eq!(num, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("jwst encode var_int (64 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..(BENCHMARK_SIZE as i64) {
                    write_var_i64(&mut encoder, i).unwrap();
                }
            })
        });
        codec_group.bench_function("jwst decode var_int (64 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as i64) {
                write_var_i64(&mut encoder, i).unwrap();
            }

            b.iter(|| {
                let mut decoder = encoder.as_slice();
                for i in 0..(BENCHMARK_SIZE as i64) {
                    let (tail, num) = read_var_i64(decoder).unwrap();
                    decoder = tail;
                    assert_eq!(num, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("jwst encode var_int (32 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..(BENCHMARK_SIZE as i32) {
                    write_var_i64(&mut encoder, i as i64).unwrap();
                }
            })
        });
        codec_group.bench_function("jwst decode var_int (32 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as i32) {
                write_var_i64(&mut encoder, i as i64).unwrap();
            }

            b.iter(|| {
                let mut decoder = encoder.as_slice();
                for i in 0..(BENCHMARK_SIZE as i32) {
                    let (tail, num) = read_var_i64(decoder).unwrap();
                    decoder = tail;
                    assert_eq!(num as i32, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("jwst encode var_uint (32 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..BENCHMARK_SIZE {
                    write_var_u64(&mut encoder, i as u64).unwrap();
                }
            })
        });
        codec_group.bench_function("jwst decode var_uint (32 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..BENCHMARK_SIZE {
                write_var_u64(&mut encoder, i as u64).unwrap();
            }

            b.iter(|| {
                let mut decoder = encoder.as_slice();
                for i in 0..BENCHMARK_SIZE {
                    let (tail, num) = read_var_u64(decoder).unwrap();
                    decoder = tail;
                    assert_eq!(num as u32, i);
                }
            })
        });
    }

    {
        codec_group.bench_function("jwst encode var_uint (64 bit)", |b| {
            b.iter(|| {
                let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
                for i in 0..(BENCHMARK_SIZE as u64) {
                    write_var_u64(&mut encoder, i).unwrap();
                }
            })
        });

        codec_group.bench_function("jwst decode var_uint (64 bit)", |b| {
            let mut encoder = Vec::with_capacity(BENCHMARK_SIZE as usize * 8);
            for i in 0..(BENCHMARK_SIZE as u64) {
                write_var_u64(&mut encoder, i).unwrap();
            }

            b.iter(|| {
                let mut decoder = encoder.as_slice();
                for i in 0..(BENCHMARK_SIZE as u64) {
                    let (tail, num) = read_var_u64(decoder).unwrap();
                    decoder = tail;
                    assert_eq!(num, i);
                }
            })
        });
    }
}

criterion_group!(benches, codec);
criterion_main!(benches);
