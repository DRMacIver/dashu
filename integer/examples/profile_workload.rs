//! Profile target. Runs the hegel-flavoured workloads in a tight loop so
//! a sampling profiler (samply, instruments, dtrace) can attribute cycles
//! to dashu functions. Not a benchmark — the timings here aren't meaningful;
//! the goal is purely to give a profiler enough samples.
//!
//! Build with release + debug info for symbol resolution:
//!   cargo build --release -p dashu-int --example profile_workload --features rand
//! Then run under samply:
//!   samply record --save-only -o prof.json.gz \
//!     ./target/release/examples/profile_workload <scenario> <seconds>
//!
//! Scenarios:
//!   sum  — running_sum_and_compare
//!   str  — string round-trip
//!   mix  — bounded arithmetic mix

use std::env;
use std::hint::black_box;
use std::time::{Duration, Instant};

use dashu_int::{IBig, UBig};
use rand_v08::prelude::*;
use rand_v08::rngs::StdRng;

#[derive(Clone, Copy)]
enum ValueClass {
    Zero,
    OneWord,
    TwoWord,
    JustOverInline,
    Mid,
    Large,
}

fn random_ubig<R: Rng>(bits: usize, rng: &mut R) -> UBig {
    rng.gen_range(UBig::ONE << (bits - 1)..UBig::ONE << bits)
}

fn sample_ibig<R: Rng>(class: ValueClass, rng: &mut R) -> IBig {
    let mag = match class {
        ValueClass::Zero => UBig::from(0u32),
        ValueClass::OneWord => UBig::from(rng.gen::<u64>() | 1),
        ValueClass::TwoWord => {
            let lo: u64 = rng.gen();
            let hi: u64 = rng.gen::<u64>() | (1 << 63);
            (UBig::from(hi) << 64) + UBig::from(lo)
        }
        ValueClass::JustOverInline => random_ubig(192, rng),
        ValueClass::Mid => random_ubig(1024, rng),
        ValueClass::Large => random_ubig(100_000, rng),
    };
    let mag = IBig::from(mag);
    if rng.gen::<bool>() {
        -mag
    } else {
        mag
    }
}

fn mixed_class<R: Rng>(rng: &mut R) -> ValueClass {
    let r: u32 = rng.gen_range(0..100);
    match r {
        0..=4 => ValueClass::Zero,
        5..=64 => ValueClass::OneWord,
        65..=89 => ValueClass::TwoWord,
        90..=96 => ValueClass::JustOverInline,
        97..=98 => ValueClass::Mid,
        _ => ValueClass::Large,
    }
}

fn build_inputs(n: usize) -> Vec<IBig> {
    let mut rng = StdRng::seed_from_u64(0xDA5_4_BE_4);
    (0..n).map(|_| sample_ibig(mixed_class(&mut rng), &mut rng)).collect()
}

/// Small-only distribution: every value fits in i128 (no heap path).
fn build_small_inputs(n: usize) -> Vec<IBig> {
    let mut rng = StdRng::seed_from_u64(0xDA5_4_BE_4);
    (0..n)
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

fn run_sum(inputs: &[IBig], iters: usize) -> (IBig, u32) {
    let bound: IBig = IBig::from(1i64) << 200;
    let mut last = (IBig::from(0), 0u32);
    for _ in 0..iters {
        let mut sum = IBig::from(0);
        let mut hits = 0u32;
        for v in inputs {
            sum += black_box(v);
            if black_box(&sum) >= black_box(&bound) {
                hits += 1;
                sum = IBig::from(0);
            }
        }
        last = (sum, hits);
    }
    last
}

fn run_str(inputs: &[IBig], iters: usize) -> IBig {
    let mut last = IBig::from(0);
    for _ in 0..iters {
        for v in inputs {
            let s = black_box(v).to_string();
            last = s.parse().unwrap();
        }
    }
    last
}

fn run_mix(inputs: &[IBig], iters: usize) -> (IBig, IBig, IBig, IBig) {
    let mut out = (IBig::from(0), IBig::from(1), IBig::from(-1), IBig::from(2));
    for _ in 0..iters {
        let (mut r0, mut r1, mut r2, mut r3) = (
            IBig::from(0),
            IBig::from(1),
            IBig::from(-1),
            IBig::from(2),
        );
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
        out = (r0, r1, r2, r3);
    }
    out
}

fn main() {
    let mut args = env::args().skip(1);
    let scenario = args.next().unwrap_or_else(|| "sum".into());
    let seconds: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    let inputs = if scenario.ends_with("-small") {
        build_small_inputs(4096)
    } else {
        build_inputs(4096)
    };
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut total_iters: u64 = 0;

    while Instant::now() < deadline {
        match scenario.trim_end_matches("-small") {
            "sum" => {
                black_box(run_sum(&inputs, 32));
                total_iters += 32;
            }
            "str" => {
                black_box(run_str(&inputs, 4));
                total_iters += 4;
            }
            "mix" => {
                black_box(run_mix(&inputs, 4));
                total_iters += 4;
            }
            other => panic!("unknown scenario: {other}"),
        }
    }

    eprintln!(
        "scenario={scenario} inputs={} total_iterations={total_iters}",
        inputs.len()
    );
}
