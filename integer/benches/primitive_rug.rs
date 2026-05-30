//! `rug::Integer` mirror of `primitive.rs`.
//!
//! Same bit-width sweep (10^1 .. 10^6 bits), same SEED, same operations.
//! `ConstDivisor` has no rug analogue, so the modulo benches use plain
//! division/`pow_mod` — close enough for ballpark comparison since dashu's
//! `ConstDivisor::reduce` is mostly a one-shot precomputation.
//!
//! Run (requires GMP toolchain):
//!   cargo bench -p dashu-int --bench primitive_rug --features rug-bench

use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use rand_v08::prelude::*;
use rug::ops::Pow;
use rug::Integer;
use std::fmt::Write;

const SEED: u64 = 1;

/// Positive rug Integer of approximately `bits` bits. Mirrors the dashu
/// `random_ubig`: top bit forced on, magnitude in [2^(bits-1), 2^bits).
fn random_int<R: Rng + ?Sized>(bits: usize, rng: &mut R) -> Integer {
    debug_assert!(bits >= 1);
    let words = bits.div_ceil(64);
    let mut limbs: Vec<u64> = (0..words).map(|_| rng.gen()).collect();
    let top_word = (bits - 1) / 64;
    let top_bit = (bits - 1) % 64;
    limbs.truncate(top_word + 1);
    let mask = if top_bit == 63 {
        u64::MAX
    } else {
        (1u64 << (top_bit + 1)) - 1
    };
    limbs[top_word] &= mask;
    limbs[top_word] |= 1u64 << top_bit;

    let mut out = Integer::new();
    for &w in limbs.iter().rev() {
        out <<= 64;
        out += w;
    }
    out
}

macro_rules! add_binop_benchmark {
    ($name:ident, $op:tt, $max_log_bits:literal) => {
        fn $name(criterion: &mut Criterion) {
            let mut rng = StdRng::seed_from_u64(SEED);
            let mut group = criterion.benchmark_group(stringify!($name));
            group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

            for log_bits in 1..=$max_log_bits {
                let bits = 10usize.pow(log_bits);
                let a = random_int(bits, &mut rng);
                let b: Integer = Integer::from(random_int(bits, &mut rng) + &a);
                group.bench_with_input(
                    BenchmarkId::from_parameter(bits),
                    &(a, b),
                    |bencher, (ta, tb)| bencher.iter(|| Integer::from(tb $op ta)),
                );
            }

            group.finish();
        }
    };
}

add_binop_benchmark!(ubig_add, +, 6);
add_binop_benchmark!(ubig_sub, -, 6);
add_binop_benchmark!(ubig_mul, *, 6);
add_binop_benchmark!(ubig_div, /, 6);

fn ubig_gcd(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_gcd");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let b: Integer = Integer::from(random_int(bits, &mut rng) + &a);
        group.bench_with_input(
            BenchmarkId::from_parameter(bits),
            &(a, b),
            |bencher, (ta, tb)| bencher.iter(|| Integer::from(tb.gcd_ref(ta))),
        );
    }

    group.finish();
}

fn ubig_gcd_ext(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_gcd_ext");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=5 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let b: Integer = Integer::from(random_int(bits, &mut rng) + &a);
        group.bench_with_input(
            BenchmarkId::from_parameter(bits),
            &(a, b),
            |bencher, (ta, tb)| {
                bencher.iter(|| {
                    let (g, s) = <(Integer, Integer)>::from(tb.extended_gcd_ref(ta));
                    (g, s)
                })
            },
        );
    }

    group.finish();
}

fn ubig_to_hex(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_to_hex");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let mut out = String::with_capacity(bits / 4 + 1);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &a, |bencher, ta| {
            bencher.iter(|| {
                out.clear();
                write!(&mut out, "{:x}", ta).unwrap();
                out.len()
            })
        });
    }

    group.finish();
}

fn ubig_to_dec(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_to_dec");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let mut out = String::with_capacity(bits / 3 + 1);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &a, |bencher, ta| {
            bencher.iter(|| {
                out.clear();
                write!(&mut out, "{}", ta).unwrap();
                out.len()
            })
        });
    }

    group.finish();
}

fn ubig_from_hex(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_from_hex");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let s = a.to_string_radix(16);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &s, |bencher, ts| {
            bencher.iter(|| Integer::from(Integer::parse_radix(ts, 16).unwrap()))
        });
    }

    group.finish();
}

fn ubig_from_dec(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_from_dec");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let a = random_int(bits, &mut rng);
        let s = a.to_string_radix(10);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &s, |bencher, ts| {
            bencher.iter(|| Integer::from(Integer::parse_radix(ts, 10).unwrap()))
        });
    }

    group.finish();
}

fn ubig_pow(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("ubig_pow");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_power in 1..=6 {
        let p = 10u32.pow(log_power);
        group.bench_with_input(BenchmarkId::from_parameter(p), &p, |bencher, p| {
            bencher.iter(|| Integer::from(Integer::from(3).pow(*p)))
        });
    }

    group.finish();
}

fn ubig_modulo_mul(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_modulo_mul");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=6 {
        let bits = 10usize.pow(log_bits);
        let m = random_int(bits, &mut rng);
        let a = Integer::from(random_int(bits, &mut rng) % &m);
        let b = Integer::from(random_int(bits, &mut rng) % &m);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &(a, b, m), |bencher, (ta, tb, tm)| {
            bencher.iter(|| Integer::from(ta * tb) % tm)
        });
    }

    group.finish();
}

fn ubig_modulo_pow(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut group = criterion.benchmark_group("ubig_modulo_pow");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for log_bits in 1..=4 {
        if log_bits == 4 {
            group.sample_size(10);
        }
        let bits = 10usize.pow(log_bits);
        let m = random_int(bits, &mut rng);
        let a = Integer::from(random_int(2048, &mut rng) % &m);
        let b = random_int(bits, &mut rng);
        group.bench_with_input(BenchmarkId::from_parameter(bits), &(a, b, m), |bencher, (ta, tb, tm)| {
            bencher.iter(|| Integer::from(ta.pow_mod_ref(tb, tm).unwrap()))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    ubig_add,
    ubig_sub,
    ubig_mul,
    ubig_div,
    ubig_gcd,
    ubig_gcd_ext,
    ubig_to_hex,
    ubig_to_dec,
    ubig_from_hex,
    ubig_from_dec,
    ubig_pow,
    ubig_modulo_mul,
    ubig_modulo_pow,
);

criterion_main!(benches);
