//! `rug::Integer` mirror of `hegel_shrinker.rs`.
//!
//! Same benches, same seeds, same shapes. The only divergence: where the dashu
//! version uses `UBig` (truly unsigned) for the sort-key magnitude, rug only
//! has a single signed `Integer`, so we use `.abs()` to model the magnitude.
//! That keeps the operation shape comparable.
//!
//! Run (requires GMP toolchain):
//!   cargo bench -p dashu-int --bench hegel_shrinker_rug --features rug-bench

#[path = "common/mod.rs"]
mod common;

use common::{sample_rug_int, sample_rug_uint, seeded_rng, ValueClass};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rug::Integer;

fn ibig_clone_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_clone");
    for &class in ValueClass::ALL {
        let inputs: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &inputs,
            |b, v| {
                let mut i = 0usize;
                b.iter(|| {
                    let x = &v[i & 31];
                    i = i.wrapping_add(1);
                    black_box(x).clone()
                })
            },
        );
    }
    group.finish();
}

fn choice_node_clone(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("choice_node_clone");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let nodes: Vec<(Integer, Integer, Integer, Integer)> = (0..32)
            .map(|_| {
                let min = sample_rug_int(class, &mut rng);
                let max = sample_rug_int(class, &mut rng);
                let towards = Integer::new();
                let value = sample_rug_int(class, &mut rng);
                (min, max, towards, value)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &nodes,
            |b, n| {
                let mut i = 0usize;
                b.iter(|| {
                    let (min, max, towards, value) = &n[i & 31];
                    i = i.wrapping_add(1);
                    (
                        black_box(min).clone(),
                        black_box(max).clone(),
                        black_box(towards).clone(),
                        black_box(value).clone(),
                    )
                })
            },
        );
    }
    group.finish();
}

fn ibig_drop_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_drop");
    for &class in ValueClass::ALL {
        let templates: Vec<Integer> = (0..32).map(|_| sample_rug_int(class, &mut rng)).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &templates,
            |b, t| {
                let mut i = 0usize;
                b.iter(|| {
                    let x = t[i & 31].clone();
                    i = i.wrapping_add(1);
                    drop(black_box(x));
                })
            },
        );
    }
    group.finish();
}

fn ibig_sub_magnitude(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_sub_magnitude");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_int(class, &mut rng), sample_rug_int(class, &mut rng)))
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (value, target) = &p[i & 31];
                    i = i.wrapping_add(1);
                    Integer::from(black_box(value) - black_box(target)).abs()
                })
            },
        );
    }
    group.finish();
}

fn ibig_clamp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_clamp");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let triples: Vec<(Integer, Integer, Integer)> = (0..32)
            .map(|_| {
                let mut vals = [
                    sample_rug_int(class, &mut rng),
                    sample_rug_int(class, &mut rng),
                    sample_rug_int(class, &mut rng),
                ];
                vals.sort();
                let [min, value, max] = vals;
                (min, value, max)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &triples,
            |b, t| {
                let mut i = 0usize;
                b.iter(|| {
                    let (min, value, max) = &t[i & 31];
                    i = i.wrapping_add(1);
                    black_box(value).clone().clamp(min, max)
                })
            },
        );
    }
    group.finish();
}

fn ibig_double_cmp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_double_cmp");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let triples: Vec<(Integer, Integer, Integer)> = (0..32)
            .map(|_| {
                let a = sample_rug_int(class, &mut rng);
                let b = sample_rug_int(class, &mut rng);
                let c = sample_rug_int(class, &mut rng);
                (a, b, c)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &triples,
            |b, t| {
                let mut i = 0usize;
                b.iter(|| {
                    let (min, value, max) = &t[i & 31];
                    i = i.wrapping_add(1);
                    black_box(min) <= black_box(value) && black_box(value) <= black_box(max)
                })
            },
        );
    }
    group.finish();
}

fn ibig_from_small_consts(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibig_from_const");
    group.bench_function("zero", |b| {
        b.iter(|| Integer::from(black_box(0i64)))
    });
    group.bench_function("one", |b| {
        b.iter(|| Integer::from(black_box(1i64)))
    });
    group.bench_function("minus_one", |b| {
        b.iter(|| Integer::from(black_box(-1i64)))
    });
    group.finish();
}

fn ubig_cmp_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_cmp_shrinker");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| (sample_rug_uint(class, &mut rng), sample_rug_uint(class, &mut rng)))
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (a, c) = &p[i & 31];
                    i = i.wrapping_add(1);
                    black_box(a).cmp(black_box(c))
                })
            },
        );
    }
    group.finish();
}

fn ibig_shift_right_descent(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_shr_descent");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| {
                let lo = sample_rug_int(class, &mut rng);
                let dist = sample_rug_int(class, &mut rng).abs();
                (lo, dist)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (lo, dist) = &p[i & 31];
                    i = i.wrapping_add(1);
                    let mut last = lo.clone();
                    for k in [1u32, 2, 4, 8, 16] {
                        last = Integer::from(lo + Integer::from(black_box(dist) >> k));
                    }
                    last
                })
            },
        );
    }
    group.finish();
}

fn shrinker_consider_workload(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("shrinker_consider");

    for n_nodes in [4, 16, 64] {
        let nodes: Vec<(Integer, Integer, Integer, Integer)> = (0..n_nodes)
            .map(|_| {
                let class = if rand_v08::Rng::gen::<bool>(&mut rng) {
                    ValueClass::OneWord
                } else {
                    ValueClass::TwoWord
                };
                let a = sample_rug_int(class, &mut rng);
                let b = sample_rug_int(class, &mut rng);
                let (min, max) = if a <= b { (a, b) } else { (b, a) };
                let towards = Integer::new();
                let value = sample_rug_int(class, &mut rng);
                (min, max, towards, value)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(n_nodes),
            &nodes,
            |b, nodes| {
                b.iter(|| {
                    let cloned: Vec<_> = nodes
                        .iter()
                        .map(|(min, max, towards, value)| {
                            (min.clone(), max.clone(), towards.clone(), value.clone())
                        })
                        .collect();

                    let sort_keys: Vec<(Integer, bool)> = cloned
                        .iter()
                        .map(|(_min, _max, towards, value)| {
                            let target = towards.clone();
                            let distance = Integer::from(black_box(value) - &target).abs();
                            let below = *value < target;
                            (distance, below)
                        })
                        .collect();

                    let mut total_order = std::cmp::Ordering::Equal;
                    for i in 0..sort_keys.len() {
                        let cmp = sort_keys[i].cmp(black_box(&sort_keys[sort_keys.len() - 1 - i]));
                        if cmp != std::cmp::Ordering::Equal {
                            total_order = cmp;
                            break;
                        }
                    }
                    (cloned, sort_keys, total_order)
                })
            },
        );
    }
    group.finish();
}

fn ubig_binary_search_step(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_binary_search_step");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let triples: Vec<(Integer, Integer, Integer)> = (0..32)
            .map(|_| {
                let a = sample_rug_uint(class, &mut rng);
                let b = sample_rug_uint(class, &mut rng);
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let above = sample_rug_uint(class, &mut rng);
                (lo, hi, above)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &triples,
            |b, t| {
                let mut i = 0usize;
                b.iter(|| {
                    let (lo, hi, above) = &t[i & 31];
                    i = i.wrapping_add(1);
                    let mid = Integer::from(lo + Integer::from(Integer::from(hi - lo) >> 1u32));
                    let total = Integer::from(std::cmp::min(&mid, black_box(above)))
                        + std::cmp::min(&mid, black_box(above));
                    (mid, Integer::from(total))
                })
            },
        );
    }
    group.finish();
}

fn ibig_hashmap_keys(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_hashmap_keys");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let keys: Vec<Integer> = (0..128).map(|_| sample_rug_int(class, &mut rng)).collect();
        let mut map: HashMap<Integer, u32> = HashMap::with_capacity(keys.len());
        for (i, k) in keys.iter().enumerate() {
            map.insert(k.clone(), i as u32);
        }
        group.bench_with_input(BenchmarkId::from_parameter(class.label()), &(keys, map), |b, (ks, m)| {
            let mut i = 0usize;
            b.iter(|| {
                let k = &ks[i & 127];
                i = i.wrapping_add(1);
                m.get(black_box(k)).copied()
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    ibig_clone_by_class,
    choice_node_clone,
    ibig_drop_by_class,
    ibig_sub_magnitude,
    ibig_clamp,
    ibig_double_cmp,
    ibig_from_small_consts,
    ubig_cmp_by_class,
    ibig_shift_right_descent,
    shrinker_consider_workload,
    ubig_binary_search_step,
    ibig_hashmap_keys,
);

criterion_main!(benches);
