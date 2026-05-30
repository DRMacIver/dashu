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

// ---------------------------------------------------------------------------
// from_index full binary search — `rug` mirror of `from_index_full_search`.
// ---------------------------------------------------------------------------

fn from_index_full_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_index_full_search");

    let above = Integer::from(i128::MAX as u128);
    let below = Integer::from(i128::MAX as u128 + 1);

    for target_frac in [0.0f64, 0.25, 0.5, 0.75, 1.0] {
        let target_idx = {
            let max_idx = Integer::from(&above + &below);
            let frac_bits = (target_frac * 1000.0) as u128;
            Integer::from(&max_idx * Integer::from(frac_bits)) / Integer::from(1000u32)
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("frac_{:.0}pct", target_frac * 100.0)),
            &target_idx,
            |b, idx| {
                b.iter(|| {
                    let one = Integer::from(1u32);
                    let mut lo = one.clone();
                    let mut hi = std::cmp::max(&above, &below).clone();
                    while lo < hi {
                        let mid = Integer::from(&lo + Integer::from(Integer::from(&hi - &lo) >> 1u32));
                        let total = Integer::from(std::cmp::min(&mid, &above))
                            + std::cmp::min(&mid, &below);
                        if Integer::from(total) >= *black_box(idx) {
                            hi = mid;
                        } else {
                            lo = mid + &one;
                        }
                    }
                    lo
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// By-ref add/sub mirrors.
// ---------------------------------------------------------------------------

fn ubig_ref_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_ref_add");
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
                    Integer::from(black_box(a) + black_box(c))
                })
            },
        );
    }
    group.finish();
}

fn ubig_ref_sub_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_ref_sub");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(Integer, Integer)> = (0..32)
            .map(|_| {
                let a = sample_rug_uint(class, &mut rng);
                let b = sample_rug_uint(class, &mut rng);
                if a >= b { (a, b) } else { (b, a) }
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (a, c) = &p[i & 31];
                    i = i.wrapping_add(1);
                    Integer::from(black_box(a) - black_box(c))
                })
            },
        );
    }
    group.finish();
}

fn ibig_boundary_sort(c: &mut Criterion) {
    let mut rng = seeded_rng();
    c.bench_function("ibig_boundary_sort", |b| {
        let min = Integer::from(i128::MIN + 1);
        let max = Integer::from(i128::MAX);

        b.iter(|| {
            let mut values = vec![min.clone(), max.clone(), Integer::new()];
            for sign in [1i128, -1] {
                for exp in 0..=128u32 {
                    let v = Integer::from(sign) * Integer::from(1u128 << exp.min(127));
                    values.push(v);
                }
            }
            values.push(Integer::from(rand_v08::Rng::gen_range(&mut rng, -10i64..10)));
            values.sort();
            values.dedup();
            black_box(values.len())
        })
    });
}

fn ubig_min_inline(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_min");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
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
                    std::cmp::min(black_box(a), black_box(c))
                })
            },
        );
    }
    group.finish();
}

fn integer_choice_to_index(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("integer_choice_to_index");

    let scenarios: [(&str, Integer, Integer, Integer); 2] = [
        (
            "i128_range",
            Integer::from(i128::MIN + 1),
            Integer::from(0),
            Integer::from(i128::MAX),
        ),
        (
            "heap_range",
            Integer::from(0),
            Integer::from(0),
            Integer::from(1u128) << 200,
        ),
    ];

    for (label, min_v, s, max_v) in scenarios.iter() {
        let values: Vec<Integer> = (0..32)
            .map(|i| {
                let class = match i % 4 {
                    0 => ValueClass::OneWord,
                    1 => ValueClass::TwoWord,
                    _ => ValueClass::OneWord,
                };
                let mag = sample_rug_int(class, &mut rng);
                if &mag > max_v {
                    max_v.clone()
                } else if &mag < min_v {
                    min_v.clone()
                } else {
                    mag
                }
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(min_v.clone(), s.clone(), max_v.clone(), values),
            |b, (min_v, s, max_v, values)| {
                let mut i = 0usize;
                let one = Integer::from(1u32);
                b.iter(|| {
                    let v = &values[i & 31];
                    i = i.wrapping_add(1);
                    if v == s {
                        Integer::from(0)
                    } else {
                        let above = Integer::from(max_v - s).abs();
                        let below = Integer::from(s - min_v).abs();
                        let d_abs = Integer::from(v - s).abs();
                        let d_minus_one = Integer::from(&d_abs - &one);
                        let mut count = Integer::from(std::cmp::min(&d_minus_one, &above))
                            + std::cmp::min(&d_minus_one, &below);
                        if v > s {
                            return Integer::from(count + &one);
                        }
                        if d_abs <= above {
                            count += Integer::from(1u32);
                        }
                        Integer::from(count + Integer::from(1u32))
                    }
                })
            },
        );
    }
    group.finish();
}

fn nodes_sort_key_lex_cmp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("nodes_sort_key_lex_cmp");

    type Node = (Integer, Integer);

    fn sort_key(node: &Node) -> (Integer, bool) {
        let (value, target) = node;
        (Integer::from(value - target).abs(), value < target)
    }

    fn lex_cmp(a: &[Node], b: &[Node]) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match a.len().cmp(&b.len()) {
            Ordering::Equal => {}
            ord => return ord,
        }
        for (x, y) in a.iter().zip(b.iter()) {
            let key_x = sort_key(x);
            let key_y = sort_key(y);
            match (&key_x.0, key_x.1).cmp(&(&key_y.0, key_y.1)) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }
        Ordering::Equal
    }

    for n_nodes in [4usize, 16, 64] {
        let a: Vec<Node> = (0..n_nodes)
            .map(|_| (sample_rug_int(ValueClass::OneWord, &mut rng), Integer::new()))
            .collect();
        let mut b = a.clone();
        let mid = n_nodes / 2;
        b[mid].0 = Integer::from(&b[mid].0 + Integer::from(1));

        group.bench_with_input(
            BenchmarkId::new("same_prefix", n_nodes),
            &(a, b),
            |bn, (a, b)| {
                bn.iter(|| lex_cmp(black_box(a), black_box(b)));
            },
        );

        let a: Vec<Node> = (0..n_nodes)
            .map(|_| (sample_rug_int(ValueClass::TwoWord, &mut rng), Integer::new()))
            .collect();
        let mut b = a.clone();
        b[0].0 = Integer::from(&b[0].0 + Integer::from(1));
        group.bench_with_input(
            BenchmarkId::new("differ_at_zero", n_nodes),
            &(a, b),
            |bn, (a, b)| {
                bn.iter(|| lex_cmp(black_box(a), black_box(b)));
            },
        );
    }
    group.finish();
}

fn shrinker_descent_subtract(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("shrinker_descent_subtract");

    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let triples: Vec<(Integer, Integer, Integer)> = (0..32)
            .map(|_| {
                let base = sample_rug_int(class, &mut rng);
                let min = Integer::from(&base - Integer::from(1024i64));
                let max = Integer::from(&base + Integer::from(1024i64));
                (base, min, max)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &triples,
            |b, t| {
                let mut i = 0usize;
                const STEPS: [u64; 9] = [1, 2, 3, 4, 8, 16, 32, 64, 128];
                b.iter(|| {
                    let (base, min, max) = &t[i & 31];
                    i = i.wrapping_add(1);
                    let mut valid_count = 0u32;
                    for n in STEPS {
                        let cand = Integer::from(base - Integer::from(2u64 * n));
                        if &cand >= black_box(min) && &cand <= black_box(max) {
                            valid_count += 1;
                        }
                        let cand = Integer::from(base - Integer::from(n));
                        if &cand >= black_box(min) && &cand <= black_box(max) {
                            valid_count += 1;
                        }
                    }
                    valid_count
                })
            },
        );
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
    from_index_full_search,
    ubig_ref_add_by_class,
    ubig_ref_sub_by_class,
    ibig_boundary_sort,
    ubig_min_inline,
    integer_choice_to_index,
    nodes_sort_key_lex_cmp,
    shrinker_descent_subtract,
);

criterion_main!(benches);
