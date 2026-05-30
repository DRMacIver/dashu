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

use common::{mixed_class, sample_ibig, seeded_rng, ValueClass};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dashu_int::IBig;
use rand_v08::Rng;

const N: usize = 4096;

fn build_mixed_inputs() -> Vec<IBig> {
    let mut rng = seeded_rng();
    (0..N).map(|_| sample_ibig(mixed_class(&mut rng), &mut rng)).collect()
}

/// Small-only input distribution (every value ≤ 128 bits, all inline in `Repr`).
/// Roughly 1/3 TwoWord, 2/3 OneWord — same proportions as the `profile_workload`
/// `-small` scenarios so the bench numbers track the profile measurements.
fn build_small_inputs() -> Vec<IBig> {
    let mut rng = seeded_rng();
    (0..N)
        .map(|_| {
            let class = if rng.gen::<u32>() % 3 == 0 {
                ValueClass::TwoWord
            } else {
                ValueClass::OneWord
            };
            sample_ibig(class, &mut rng)
        })
        .collect()
}

/// Sub-1-kbit mixed-class distribution: same shape as `mixed_class` (small
/// values dominate) but bounded to ≤ 256 bits, so no operand ever pushes the
/// accumulator past the 1-kbit regime. The 2 % `Mid` (1024-bit, right at the
/// boundary) and 1 % `Large` (100 kbit) slots of `mixed_class` get
/// redistributed to `JustOverInline` (192-bit) and the inline classes — that
/// keeps the mixed-scale flavour without inviting GMP's asymptotic kernels
/// into the bench.
fn under_1kbit_class<R: Rng>(rng: &mut R) -> ValueClass {
    let r: u32 = rng.gen_range(0..100);
    match r {
        0..=4 => ValueClass::Zero,         // 5 %
        5..=64 => ValueClass::OneWord,     // 60 %
        65..=89 => ValueClass::TwoWord,    // 25 %
        _ => ValueClass::JustOverInline,   // 10 % (was 7 % + redirected Mid/Large)
    }
}

fn build_under_1kbit_inputs() -> Vec<IBig> {
    let mut rng = seeded_rng();
    (0..N).map(|_| sample_ibig(under_1kbit_class(&mut rng), &mut rng)).collect()
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

/// Scenario 1-small: running-sum-and-compare over ≤ 128-bit inputs.
///
/// Mirrors the `sum-small` profile scenario: every RHS is inline but the
/// accumulator can grow heap-resident, so the dominant cost is the per-step
/// `Repr::from_buffer` finalisation on the AddAssign path. Headline regression
/// detector for the AddAssign specialisation work in
/// `notes/2026-05-30-initial-profile.md`.
fn running_sum_and_compare_small(c: &mut Criterion) {
    let inputs = build_small_inputs();
    let bound: IBig = IBig::from(1i64) << 200;
    c.bench_function("running_sum_and_compare_small", |b| {
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

/// Scenario 3-small: scripted arithmetic mix over ≤ 128-bit inputs.
///
/// Mirrors the `mix-small` profile scenario, which surfaced a surprising
/// 17 % `_platform_memmove` cost even though no operand exceeds the inline
/// boundary. Same scripted op pattern as `bounded_arithmetic_mix`, but the
/// inputs never push registers into the large-buffer paths, so movement of
/// inline limbs (`Buffer::from(&[Word])`, `shrink_to_fit` realloc) is the
/// only plausible source of memmove cost. Regression detector for item 3
/// in the recommendations.
/// Scenario 1-under_1kbit: running-sum-and-compare strictly bounded to
/// values ≤ 256 bits. The `_small` variant covers the all-inline case; this
/// one covers the more interesting "mostly inline, occasionally just-over-
/// inline heap" regime that the user's < 1-kbit performance target is
/// directly about.
fn running_sum_and_compare_under_1kbit(c: &mut Criterion) {
    let inputs = build_under_1kbit_inputs();
    let bound: IBig = IBig::from(1i64) << 200;
    c.bench_function("running_sum_and_compare_under_1kbit", |b| {
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

/// Scripted arithmetic mix where every register stays bounded.
///
/// The earlier `bounded_arithmetic_mix*` benches grow `r2` unboundedly via
/// `r2 = &r2 * v` (and `r3` via `r3 << 1`), so by the end of a single
/// `b.iter` invocation `r2` is ~32 kbit — well outside the user's < 1 kbit
/// target. Here every reassignment writes a result whose magnitude is
/// bounded by `O(input_size)`: multiplication is between two fresh inputs
/// (≤ 512 bits), shifts and bitwise ops can only grow by one bit per op,
/// and the chained sums/diffs use freshly-drawn inputs as one operand. All
/// four registers therefore stay under 1 kbit for the entire loop.
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
                    0 => r0 = &r1 - black_box(v),
                    1 => r1 = &r0 ^ &r2,
                    2 => r2 = black_box(v) - &r3,
                    3 => r3 = &r0 & black_box(v),
                    4 => r0 = &r2 + black_box(v),
                    5 => r1 = &r3 << 1,
                    6 => r2 = black_box(v) * w,
                    _ => r3 = &r1 - &r0,
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
    running_sum_and_compare_small,
    running_sum_and_compare_under_1kbit,
    string_round_trip,
    bounded_arithmetic_mix,
    bounded_arithmetic_mix_small,
    bounded_arithmetic_mix_under_1kbit,
);

criterion_main!(benches);
