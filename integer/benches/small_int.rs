//! Fast-path / inline-representation benchmarks.
//!
//! The existing `primitive.rs` benchmarks sweep over bit widths 10..=10^6 and
//! so under-cover the small-integer path (≤ 128 bits, inline in `Repr`). This
//! file fills that gap: each group runs across every `ValueClass`, including
//! `Zero`, `OneWord`, and `TwoWord` which never appear in `primitive.rs`.
//!
//! Run:
//!   cargo bench -p dashu-int --bench small_int --features rand -- --quick

#[path = "common/mod.rs"]
mod common;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use common::{sample_ibig, sample_ubig, seeded_rng, ValueClass};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use dashu_int::{IBig, UBig};

// ---- construction from primitives ----

fn from_i64(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<i64> = (0..256).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ibig_from_i64", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = inputs[i & 255];
            i = i.wrapping_add(1);
            IBig::from(black_box(v))
        })
    });
}

fn from_i128(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<i128> = (0..256).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ibig_from_i128", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = inputs[i & 255];
            i = i.wrapping_add(1);
            IBig::from(black_box(v))
        })
    });
}

fn from_u64(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<u64> = (0..256).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ubig_from_u64", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = inputs[i & 255];
            i = i.wrapping_add(1);
            UBig::from(black_box(v))
        })
    });
}

fn from_u128(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<u128> = (0..256).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ubig_from_u128", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = inputs[i & 255];
            i = i.wrapping_add(1);
            UBig::from(black_box(v))
        })
    });
}

// ---- TryInto primitives (round-trip cost) ----

fn try_into_i128(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<IBig> = (0..256).map(|_| sample_ibig(ValueClass::TwoWord, &mut rng)).collect();
    c.bench_function("ibig_try_into_i128", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = &inputs[i & 255];
            i = i.wrapping_add(1);
            let r: Result<i128, _> = black_box(v).try_into();
            r
        })
    });
}

// ---- binops parameterised by class ----

fn ubig_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                black_box(a) + black_box(c)
            })
        });
    }
    group.finish();
}

fn ubig_mul_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_mul_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                black_box(a) * black_box(c)
            })
        });
    }
    group.finish();
}

fn ibig_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_add_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(IBig, IBig)> = (0..32)
            .map(|_| (sample_ibig(class, &mut rng), sample_ibig(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                black_box(a) + black_box(c)
            })
        });
    }
    group.finish();
}

// Mixed-class: one operand drawn from a small class, the other from a larger
// one. Models the "running total += small constant" pattern that pure
// same-class benches miss.
fn ubig_add_mixed(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_mixed");
    for &bigger in &[ValueClass::JustOverInline, ValueClass::Mid, ValueClass::Large] {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| {
                (
                    sample_ubig(bigger, &mut rng),
                    sample_ubig(ValueClass::OneWord, &mut rng),
                )
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(bigger.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (a, c) = &p[i & 31];
                    i = i.wrapping_add(1);
                    black_box(a) + black_box(c)
                })
            },
        );
    }
    group.finish();
}

// ---- comparison / hash / clone (cheap operations that dominate hot loops) ----

fn ubig_eq_same_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_eq");
    for &class in ValueClass::ALL {
        // Half identical pairs, half non-equal pairs, so the benchmark sees
        // both branches of the eq fast path.
        let pairs: Vec<(UBig, UBig)> = (0..64)
            .map(|i| {
                let a = sample_ubig(class, &mut rng);
                let b = if i % 2 == 0 { a.clone() } else { sample_ubig(class, &mut rng) };
                (a, b)
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 63];
                i = i.wrapping_add(1);
                black_box(a) == black_box(c)
            })
        });
    }
    group.finish();
}

fn ubig_cmp_same_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_cmp");
    for &class in ValueClass::ALL {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                black_box(a).cmp(black_box(c))
            })
        });
    }
    group.finish();
}

fn ubig_hash_same_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_hash");
    for &class in ValueClass::ALL {
        let inputs: Vec<UBig> = (0..32).map(|_| sample_ubig(class, &mut rng)).collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &inputs, |b, v| {
            let mut i = 0usize;
            b.iter(|| {
                let x = &v[i & 31];
                i = i.wrapping_add(1);
                let mut h = DefaultHasher::new();
                black_box(x).hash(&mut h);
                h.finish()
            })
        });
    }
    group.finish();
}

fn ubig_clone_same_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_clone");
    for &class in ValueClass::ALL {
        let inputs: Vec<UBig> = (0..32).map(|_| sample_ubig(class, &mut rng)).collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &inputs, |b, v| {
            let mut i = 0usize;
            b.iter(|| {
                let x = &v[i & 31];
                i = i.wrapping_add(1);
                black_box(x).clone()
            })
        });
    }
    group.finish();
}

// ---- string round-trip (key path for hegel BigInt(String) interchange) ----

fn ibig_display_small(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<IBig> = (0..128).map(|_| sample_ibig(ValueClass::OneWord, &mut rng)).collect();
    c.bench_function("ibig_display_small", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = &inputs[i & 127];
            i = i.wrapping_add(1);
            black_box(v).to_string()
        })
    });
}

fn ibig_from_str_small(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<String> = (0..128)
        .map(|_| sample_ibig(ValueClass::OneWord, &mut rng).to_string())
        .collect();
    c.bench_function("ibig_from_str_small", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let s = &inputs[i & 127];
            i = i.wrapping_add(1);
            black_box(s).parse::<IBig>().unwrap()
        })
    });
}

criterion_group!(
    benches,
    from_i64,
    from_i128,
    from_u64,
    from_u128,
    try_into_i128,
    ubig_add_by_class,
    ubig_mul_by_class,
    ibig_add_by_class,
    ubig_add_mixed,
    ubig_eq_same_class,
    ubig_cmp_same_class,
    ubig_hash_same_class,
    ubig_clone_same_class,
    ibig_display_small,
    ibig_from_str_small,
);

criterion_main!(benches);
