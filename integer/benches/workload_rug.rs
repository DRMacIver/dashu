//! `rug::Integer` mirror of `workload.rs`.
//!
//! Same scenarios, same seeded inputs, same control flow — only the integer
//! type differs. Bench IDs match the dashu side exactly, so the two outputs
//! can be diffed by name.
//!
//! Run (requires GMP toolchain):
//!   cargo bench -p dashu-int --bench workload_rug --features rug-bench

#[path = "common/mod.rs"]
mod common;

use common::{mixed_class, sample_rug_int, seeded_rng, ValueClass};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand_v08::Rng;
use rug::Integer;

const N: usize = 4096;

fn build_mixed_inputs() -> Vec<Integer> {
    let mut rng = seeded_rng();
    (0..N).map(|_| sample_rug_int(mixed_class(&mut rng), &mut rng)).collect()
}

fn build_small_inputs() -> Vec<Integer> {
    let mut rng = seeded_rng();
    (0..N)
        .map(|_| {
            let class = if rng.gen::<u32>() % 3 == 0 {
                ValueClass::TwoWord
            } else {
                ValueClass::OneWord
            };
            sample_rug_int(class, &mut rng)
        })
        .collect()
}

fn under_1kbit_class<R: Rng>(rng: &mut R) -> ValueClass {
    let r: u32 = rng.gen_range(0..100);
    match r {
        0..=4 => ValueClass::Zero,
        5..=64 => ValueClass::OneWord,
        65..=89 => ValueClass::TwoWord,
        _ => ValueClass::JustOverInline,
    }
}

fn build_under_1kbit_inputs() -> Vec<Integer> {
    let mut rng = seeded_rng();
    (0..N).map(|_| sample_rug_int(under_1kbit_class(&mut rng), &mut rng)).collect()
}

fn running_sum_and_compare(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    let bound: Integer = Integer::from(1) << 200;
    c.bench_function("running_sum_and_compare", |b| {
        b.iter(|| {
            let mut sum = Integer::new();
            let mut hits = 0u32;
            for v in &inputs {
                sum += black_box(v);
                if black_box(&sum) >= black_box(&bound) {
                    hits += 1;
                    sum = Integer::new();
                }
            }
            (sum, hits)
        })
    });
}

fn string_round_trip(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    c.bench_function("string_round_trip", |b| {
        b.iter(|| {
            let mut last = Integer::new();
            for v in &inputs {
                let s = black_box(v).to_string();
                let parsed: Integer = s.parse().unwrap();
                last = parsed;
            }
            last
        })
    });
}

fn string_round_trip_under_1kbit(c: &mut Criterion) {
    let inputs = build_under_1kbit_inputs();
    c.bench_function("string_round_trip_under_1kbit", |b| {
        b.iter(|| {
            let mut last = Integer::new();
            for v in &inputs {
                let s = black_box(v).to_string();
                let parsed: Integer = s.parse().unwrap();
                last = parsed;
            }
            last
        })
    });
}

fn bounded_arithmetic_mix(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    c.bench_function("bounded_arithmetic_mix", |b| {
        b.iter(|| {
            let mut r0 = Integer::from(0);
            let mut r1 = Integer::from(1);
            let mut r2 = Integer::from(-1);
            let mut r3 = Integer::from(2);
            for (i, v) in inputs.iter().enumerate() {
                match i & 7 {
                    0 => r0 = Integer::from(&r0 + black_box(v)),
                    1 => r1 = Integer::from(&r1 - black_box(v)),
                    2 => r2 = Integer::from(&r2 * black_box(v)),
                    3 => r3 = Integer::from(&r3 + &r0),
                    4 => r0 = Integer::from(&r0 ^ &r1),
                    5 => r1 = Integer::from(&r2 & black_box(v)),
                    6 => r2 = Integer::from(&r3 << 1u32),
                    _ => r3 = Integer::from(&r0 + &r2),
                }
            }
            (r0, r1, r2, r3)
        })
    });
}

fn running_sum_and_compare_small(c: &mut Criterion) {
    let inputs = build_small_inputs();
    let bound: Integer = Integer::from(1) << 200;
    c.bench_function("running_sum_and_compare_small", |b| {
        b.iter(|| {
            let mut sum = Integer::new();
            let mut hits = 0u32;
            for v in &inputs {
                sum += black_box(v);
                if black_box(&sum) >= black_box(&bound) {
                    hits += 1;
                    sum = Integer::new();
                }
            }
            (sum, hits)
        })
    });
}

fn running_sum_and_compare_under_1kbit(c: &mut Criterion) {
    let inputs = build_under_1kbit_inputs();
    let bound: Integer = Integer::from(1) << 200;
    c.bench_function("running_sum_and_compare_under_1kbit", |b| {
        b.iter(|| {
            let mut sum = Integer::new();
            let mut hits = 0u32;
            for v in &inputs {
                sum += black_box(v);
                if black_box(&sum) >= black_box(&bound) {
                    hits += 1;
                    sum = Integer::new();
                }
            }
            (sum, hits)
        })
    });
}

fn bounded_arithmetic_mix_under_1kbit(c: &mut Criterion) {
    let inputs = build_under_1kbit_inputs();
    c.bench_function("bounded_arithmetic_mix_under_1kbit", |b| {
        b.iter(|| {
            let mut r0 = inputs[0].clone();
            let mut r1 = inputs[1].clone();
            let mut r2 = inputs[2].clone();
            let mut r3 = inputs[3].clone();
            for (i, v) in inputs.iter().enumerate() {
                let w = &inputs[i.wrapping_add(7) & (N - 1)];
                match i & 7 {
                    0 => r0 = Integer::from(&r1 - black_box(v)),
                    1 => r1 = Integer::from(&r0 ^ &r2),
                    2 => r2 = Integer::from(black_box(v) - &r3),
                    3 => r3 = Integer::from(&r0 & black_box(v)),
                    4 => r0 = Integer::from(&r2 + black_box(v)),
                    5 => r1 = Integer::from(&r3 << 1u32),
                    6 => r2 = Integer::from(black_box(v) * w),
                    _ => r3 = Integer::from(&r1 - &r0),
                }
            }
            (r0, r1, r2, r3)
        })
    });
}

fn bounded_arithmetic_mix_small(c: &mut Criterion) {
    let inputs = build_small_inputs();
    c.bench_function("bounded_arithmetic_mix_small", |b| {
        b.iter(|| {
            let mut r0 = inputs[0].clone();
            let mut r1 = inputs[1].clone();
            let mut r2 = inputs[2].clone();
            let mut r3 = inputs[3].clone();
            for (i, v) in inputs.iter().enumerate() {
                let w = &inputs[i.wrapping_add(7) & (N - 1)];
                match i & 7 {
                    0 => r0 = Integer::from(&r1 - black_box(v)),
                    1 => r1 = Integer::from(&r0 ^ &r2),
                    2 => r2 = Integer::from(black_box(v) - &r3),
                    3 => r3 = Integer::from(&r0 & black_box(v)),
                    4 => r0 = Integer::from(&r2 + black_box(v)),
                    5 => r1 = Integer::from(&r3 << 1u32),
                    6 => r2 = Integer::from(black_box(v) * w),
                    _ => r3 = Integer::from(&r1 - &r0),
                }
            }
            (r0, r1, r2, r3)
        })
    });
}

criterion_group!(
    benches,
    running_sum_and_compare,
    running_sum_and_compare_small,
    running_sum_and_compare_under_1kbit,
    string_round_trip,
    string_round_trip_under_1kbit,
    bounded_arithmetic_mix,
    bounded_arithmetic_mix_small,
    bounded_arithmetic_mix_under_1kbit,
);

criterion_main!(benches);
