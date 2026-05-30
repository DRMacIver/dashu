//! `rug::Integer` mirror of `small_int.rs`.
//!
//! Every bench has the same name and structure as the dashu variant in
//! `small_int.rs` so the two outputs can be diffed entry-for-entry. Inputs
//! are drawn from the same RNG seed (`seeded_rng`) at the same `ValueClass`
//! magnitudes, so the comparison is apples-to-apples per parameter point.
//!
//! Run (requires GMP toolchain):
//!   cargo bench -p dashu-int --bench small_int_rug --features rug-bench

#[path = "common/mod.rs"]
mod common;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use common::{sample_rug_int, sample_rug_uint, seeded_rng, ValueClass};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rug::Integer;

// ---- construction from primitives ----

fn from_i64(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<i64> = (0..256).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ibig_from_i64", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = inputs[i & 255];
            i = i.wrapping_add(1);
            Integer::from(black_box(v))
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
            Integer::from(black_box(v))
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
            Integer::from(black_box(v))
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
            Integer::from(black_box(v))
        })
    });
}

// ---- TryInto primitives (round-trip cost) ----

fn try_into_i128(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<Integer> = (0..256)
        .map(|_| sample_rug_int(ValueClass::TwoWord, &mut rng))
        .collect();
    c.bench_function("ibig_try_into_i128", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let v = &inputs[i & 255];
            i = i.wrapping_add(1);
            let r: Result<i128, _> = i128::try_from(black_box(v));
            r
        })
    });
}

// ---- binops parameterised by class ----

fn ubig_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_uint(class, &mut rng), sample_rug_uint(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                Integer::from(black_box(a) + black_box(c))
            })
        });
    }
    group.finish();
}

fn ubig_mul_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_mul_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_uint(class, &mut rng), sample_rug_uint(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                Integer::from(black_box(a) * black_box(c))
            })
        });
    }
    group.finish();
}

fn ibig_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_add_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_int(class, &mut rng), sample_rug_int(class, &mut rng)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                Integer::from(black_box(a) + black_box(c))
            })
        });
    }
    group.finish();
}

fn ubig_add_mixed(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_mixed");
    for &bigger in &[ValueClass::JustOverInline, ValueClass::Mid, ValueClass::Large] {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| {
                (
                    sample_rug_uint(bigger, &mut rng),
                    sample_rug_uint(ValueClass::OneWord, &mut rng),
                )
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(bigger.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (a, c) = &p[i & 31];
                i = i.wrapping_add(1);
                Integer::from(black_box(a) + black_box(c))
            })
        });
    }
    group.finish();
}

// ---- assign-form binops ----

fn ubig_add_assign_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_assign_by_class");
    for &class in ValueClass::ALL {
        let starts: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &(starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc += black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

fn ibig_add_assign_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_add_assign_by_class");
    for &class in ValueClass::ALL {
        let starts: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &(starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc += black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

fn ubig_sub_assign_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_sub_assign_by_class");
    for &class in ValueClass::ALL {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| {
                let a = sample_rug_uint(class, &mut rng);
                let b = sample_rug_uint(class, &mut rng);
                (Integer::from(&a + &b), b)
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &pairs, |b, p| {
            let mut i = 0usize;
            b.iter(|| {
                let (start, rhs) = &p[i & 31];
                let mut acc = start.clone();
                acc -= black_box(rhs);
                i = i.wrapping_add(1);
                acc
            })
        });
    }
    group.finish();
}

fn ibig_sub_assign_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_sub_assign_by_class");
    for &class in ValueClass::ALL {
        let starts: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &(starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc -= black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

fn ubig_add_assign_heap_acc_small_rhs(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_add_assign_heap_acc_small_rhs");
    for &acc_class in &[ValueClass::JustOverInline, ValueClass::Mid, ValueClass::Large] {
        let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_uint(acc_class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_uint(ValueClass::OneWord, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(acc_class.label()),
            &(acc_starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc += black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

fn ibig_add_assign_heap_acc_small_rhs(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_add_assign_heap_acc_small_rhs");
    for &acc_class in &[ValueClass::JustOverInline, ValueClass::Mid, ValueClass::Large] {
        let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_int(acc_class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_int(ValueClass::OneWord, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(acc_class.label()),
            &(acc_starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc += black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

fn ibig_add_assign_i64_into_heap_acc(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_int(ValueClass::Mid, &mut rng)).collect();
    let rhs: Vec<i64> = (0..32).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ibig_add_assign_i64_into_heap_acc", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let mut acc = acc_starts[i & 31].clone();
            acc += black_box(rhs[i & 31]);
            i = i.wrapping_add(1);
            acc
        })
    });
}

fn ibig_add_assign_i128_into_heap_acc(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_int(ValueClass::Mid, &mut rng)).collect();
    let rhs: Vec<i128> = (0..32).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ibig_add_assign_i128_into_heap_acc", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let mut acc = acc_starts[i & 31].clone();
            acc += black_box(rhs[i & 31]);
            i = i.wrapping_add(1);
            acc
        })
    });
}

fn ubig_add_assign_u64_into_heap_acc(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_uint(ValueClass::Mid, &mut rng)).collect();
    let rhs: Vec<u64> = (0..32).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ubig_add_assign_u64_into_heap_acc", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let mut acc = acc_starts[i & 31].clone();
            acc += black_box(rhs[i & 31]);
            i = i.wrapping_add(1);
            acc
        })
    });
}

fn ubig_add_assign_u128_into_heap_acc(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let acc_starts: Vec<Integer> = (0..32).map(|_| sample_rug_uint(ValueClass::Mid, &mut rng)).collect();
    let rhs: Vec<u128> = (0..32).map(|_| rand_v08::Rng::gen(&mut rng)).collect();
    c.bench_function("ubig_add_assign_u128_into_heap_acc", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let mut acc = acc_starts[i & 31].clone();
            acc += black_box(rhs[i & 31]);
            i = i.wrapping_add(1);
            acc
        })
    });
}

fn ubig_bitxor_assign_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_bitxor_assign_by_class");
    for &class in ValueClass::ALL {
        let starts: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
        let rhs: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &(starts, rhs),
            |b, (s, r)| {
                let mut i = 0usize;
                b.iter(|| {
                    let mut acc = s[i & 31].clone();
                    acc ^= black_box(&r[i & 31]);
                    i = i.wrapping_add(1);
                    acc
                })
            },
        );
    }
    group.finish();
}

// ---- comparison / hash / clone ----

fn ubig_eq_same_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_eq");
    for &class in ValueClass::ALL {
        let pairs: Vec<(Integer, Integer)> = (0..64)
            .map(|i| {
                let a = sample_rug_uint(class, &mut rng);
                let b = if i % 2 == 0 { a.clone() } else { sample_rug_uint(class, &mut rng) };
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
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_uint(class, &mut rng), sample_rug_uint(class, &mut rng)))
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
        let inputs: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
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
        let inputs: Vec<Integer> = (0..32).map(|_| sample_rug_uint(class, &mut rng)).collect();
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

// ---- string round-trip ----

fn ibig_display_small(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let inputs: Vec<Integer> = (0..128).map(|_| sample_rug_int(ValueClass::OneWord, &mut rng)).collect();
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
        .map(|_| sample_rug_int(ValueClass::OneWord, &mut rng).to_string())
        .collect();
    c.bench_function("ibig_from_str_small", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let s = &inputs[i & 127];
            i = i.wrapping_add(1);
            black_box(s).parse::<Integer>().unwrap()
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
    ubig_add_assign_by_class,
    ibig_add_assign_by_class,
    ubig_sub_assign_by_class,
    ibig_sub_assign_by_class,
    ubig_add_assign_heap_acc_small_rhs,
    ibig_add_assign_heap_acc_small_rhs,
    ibig_add_assign_i64_into_heap_acc,
    ibig_add_assign_i128_into_heap_acc,
    ubig_add_assign_u64_into_heap_acc,
    ubig_add_assign_u128_into_heap_acc,
    ubig_bitxor_assign_by_class,
    ubig_eq_same_class,
    ubig_cmp_same_class,
    ubig_hash_same_class,
    ubig_clone_same_class,
    ibig_display_small,
    ibig_from_str_small,
);

criterion_main!(benches);
