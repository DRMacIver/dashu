//! Hegel-flavoured scenario benchmarks.
//!
//! Three scenarios shaped by what `hegel-rust` does to bigints in practice:
//! mostly-small values with occasional larger ones, lots of comparisons,
//! and frequent `BigInt(String)` interchange round-trips.
//!
//! Run:
//!   cargo bench -p dashu-int --bench workload --features rand -- --quick

#[path = "common/mod.rs"]
mod common;

use common::{mixed_class, sample_ibig, seeded_rng};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dashu_int::IBig;

const N: usize = 4096;

fn build_mixed_inputs() -> Vec<IBig> {
    let mut rng = seeded_rng();
    (0..N).map(|_| sample_ibig(mixed_class(&mut rng), &mut rng)).collect()
}

/// Scenario 1: running-sum-and-compare.
///
/// Mirrors the targeting/score loop in hegel generators: every step adds the
/// next value into an accumulator and checks whether it crossed a bound.
/// Most inputs fit in i64 (per `mixed_class`); the accumulator may grow.
fn running_sum_and_compare(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    let bound: IBig = IBig::from(1i64) << 200;
    c.bench_function("running_sum_and_compare", |b| {
        b.iter(|| {
            let mut sum = IBig::from(0);
            let mut hits = 0u32;
            for v in &inputs {
                sum += black_box(v);
                if black_box(&sum) >= black_box(&bound) {
                    hits += 1;
                    sum = IBig::from(0);
                }
            }
            (sum, hits)
        })
    });
}

/// Scenario 2: string round-trip.
///
/// `HegelValue::BigInt(String)` (see hegel-rust src/generators/value.rs) means
/// every bigint that crosses the protocol boundary is formatted to and parsed
/// from decimal. This bench measures the steady-state cost of that path on
/// mostly-small values.
fn string_round_trip(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    c.bench_function("string_round_trip", |b| {
        b.iter(|| {
            let mut last = IBig::from(0);
            for v in &inputs {
                let s = black_box(v).to_string();
                let parsed: IBig = s.parse().unwrap();
                last = parsed;
            }
            last
        })
    });
}

/// Scenario 3: bounded arithmetic mix.
///
/// A scripted sequence of `+`, `-`, `*`, `<<`, `&` over a small working set.
/// Simulates one step of stateful test execution where most intermediate
/// values stay inline. The exact op sequence is fixed so successive runs
/// are comparable.
fn bounded_arithmetic_mix(c: &mut Criterion) {
    let inputs = build_mixed_inputs();
    c.bench_function("bounded_arithmetic_mix", |b| {
        b.iter(|| {
            // Four live registers, refreshed periodically from `inputs`.
            let mut r0 = IBig::from(0);
            let mut r1 = IBig::from(1);
            let mut r2 = IBig::from(-1);
            let mut r3 = IBig::from(2);
            for (i, v) in inputs.iter().enumerate() {
                match i & 7 {
                    0 => r0 = &r0 + black_box(v),
                    1 => r1 = &r1 - black_box(v),
                    2 => r2 = &r2 * black_box(v),
                    3 => r3 = &r3 + &r0,
                    4 => r0 = &r0 ^ &r1,
                    5 => r1 = &r2 & black_box(v),
                    6 => r2 = &r3 << 1,
                    _ => r3 = &r0 + &r2,
                }
            }
            (r0, r1, r2, r3)
        })
    });
}

// TODO: a fourth scenario derived from a real `generic-ints` trace once the
// repo is available locally.

criterion_group!(
    benches,
    running_sum_and_compare,
    string_round_trip,
    bounded_arithmetic_mix,
);

criterion_main!(benches);
