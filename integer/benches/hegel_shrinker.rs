//! Benchmarks modelling the dashu-int operations that dominate hegel-rust's
//! shrinker hot path.
//!
//! Profiling the hegel-rust native test suite with callgrind against this
//! branch shows:
//!
//! | Operation           | % of total Ir | Notes                                   |
//! |---------------------|---------------|-----------------------------------------|
//! | Repr::clone         |  13.24 %      | Cloning IntegerChoice (3 IBig fields)   |
//! | IBig::sub           |   2.2 %       | sort_key: `value - target`              |
//! | UBig::add           |   1.0 %       | sort_key / to_index accumulation        |
//! | IBig cmp/clamp      |   1.1 %       | clamped_shrink_towards, validate        |
//! | Drop(Repr)          |   0.5 %       | Dropping ChoiceNode / ChoiceKind        |
//!
//! Values are almost always in the OneWord or TwoWord range (≤ 128 bits,
//! inline in Repr). The clone cost comes from volume: the shrinker clones
//! entire ChoiceNode vectors thousands of times per shrink run, each node
//! containing three IBig (min, max, shrink_towards) plus one IBig value.
//!
//! Run:
//!   cargo bench -p dashu-int --bench hegel_shrinker --features rand

#[path = "common/mod.rs"]
mod common;

use common::{sample_ibig, sample_ubig, seeded_rng, ValueClass};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dashu_int::{IBig, UBig};
use dashu_int::ops::UnsignedAbs;

// ---------------------------------------------------------------------------
// 1. Clone — the dominant bottleneck (13 % of shrinker Ir).
//
// Each ChoiceNode contains an IntegerChoice (min, max, shrink_towards: 3×IBig)
// plus a ChoiceValue (1×IBig). The shrinker clones entire Vec<ChoiceNode> on
// every consider() call, so the per-IBig clone cost is multiplied by
// 4 * n_nodes * n_candidates.
// ---------------------------------------------------------------------------

fn ibig_clone_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_clone");
    for &class in ValueClass::ALL {
        let inputs: Vec<IBig> = (0..32).map(|_| sample_ibig(class, &mut rng)).collect();
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

/// Clone a "ChoiceNode"-shaped struct: 3 IBig fields (min, max, shrink_towards)
/// + 1 IBig value.  This is the atomic unit the shrinker clones; measuring it
/// directly captures the aggregate overhead better than per-field clones.
fn choice_node_clone(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("choice_node_clone");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let nodes: Vec<(IBig, IBig, IBig, IBig)> = (0..32)
            .map(|_| {
                let min = sample_ibig(class, &mut rng);
                let max = sample_ibig(class, &mut rng);
                let towards = IBig::from(0);
                let value = sample_ibig(class, &mut rng);
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

// ---------------------------------------------------------------------------
// 2. Drop — paired with clone, every cloned value is eventually dropped.
//    The shrinker clones a Vec<ChoiceNode>, evaluates it, then drops it.
// ---------------------------------------------------------------------------

fn ibig_drop_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_drop");
    for &class in ValueClass::ALL {
        let templates: Vec<IBig> = (0..32).map(|_| sample_ibig(class, &mut rng)).collect();
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

// ---------------------------------------------------------------------------
// 3. sort_key pattern: `(value - target).magnitude()`.
//    Called once per node per consider(), so ~n_nodes * n_candidates times
//    per shrink run. The sub + magnitude pair is 1.28 % + 0.5 % of Ir.
// ---------------------------------------------------------------------------

fn ibig_sub_magnitude(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_sub_magnitude");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(IBig, IBig)> = (0..32)
            .map(|_| (sample_ibig(class, &mut rng), sample_ibig(class, &mut rng)))
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &pairs,
            |b, p| {
                let mut i = 0usize;
                b.iter(|| {
                    let (value, target) = &p[i & 31];
                    i = i.wrapping_add(1);
                    (black_box(value) - black_box(target)).unsigned_abs()
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 4. clamped_shrink_towards: `value.clamp(min, max)`.
//    0.57 % of Ir. Uses Ord::clamp which does two comparisons + one clone.
// ---------------------------------------------------------------------------

fn ibig_clamp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_clamp");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let triples: Vec<(IBig, IBig, IBig)> = (0..32)
            .map(|_| {
                let mut vals = [
                    sample_ibig(class, &mut rng),
                    sample_ibig(class, &mut rng),
                    sample_ibig(class, &mut rng),
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
                    black_box(value).clone().clamp(min.clone(), max.clone())
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 5. validate: `min <= value && value <= max`.
//    Two comparisons per validate(), called from replace() on every candidate.
// ---------------------------------------------------------------------------

fn ibig_double_cmp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_double_cmp");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let triples: Vec<(IBig, IBig, IBig)> = (0..32)
            .map(|_| {
                let a = sample_ibig(class, &mut rng);
                let b = sample_ibig(class, &mut rng);
                let c = sample_ibig(class, &mut rng);
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

// ---------------------------------------------------------------------------
// 6. IBig from small primitives — used to construct BigInt::from(0),
//    BigInt::from(1), BigInt::from(n as u64) throughout the shrinker.
// ---------------------------------------------------------------------------

fn ibig_from_small_consts(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibig_from_const");
    group.bench_function("zero", |b| {
        b.iter(|| IBig::from(black_box(0i64)))
    });
    group.bench_function("one", |b| {
        b.iter(|| IBig::from(black_box(1i64)))
    });
    group.bench_function("minus_one", |b| {
        b.iter(|| IBig::from(black_box(-1i64)))
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 7. UBig::cmp — used in NodeSortKeyRef::cmp on the sort keys (BigUint
//    distances). 0.22 % of Ir for the comparisons themselves.
// ---------------------------------------------------------------------------

fn ubig_cmp_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_cmp_shrinker");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
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

// ---------------------------------------------------------------------------
// 8. Shift-right descent — the shrinker's binary search uses
//    `lo + (dist >> k as usize)` where k grows geometrically.
// ---------------------------------------------------------------------------

fn ibig_shift_right_descent(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_shr_descent");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let pairs: Vec<(IBig, IBig)> = (0..32)
            .map(|_| {
                let lo = sample_ibig(class, &mut rng);
                let dist = sample_ibig(class, &mut rng).unsigned_abs();
                (lo, IBig::from(dist))
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
                    // Simulate the find_integer inner loop: lo + (dist >> k as usize)
                    // for k = 1, 2, 4, 8, 16
                    let mut last = lo.clone();
                    for k in [1u32, 2, 4, 8, 16] {
                        last = lo + (black_box(dist) >> k as usize);
                    }
                    last
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 9. Shrinker workload — simulates a single consider() call's hot path:
//    clone n nodes (each 4 IBig), compute sort_key for each (sub + magnitude),
//    then compare sort key sequences lexicographically.
//
//    This is the top-level scenario bench that combines all the above.
// ---------------------------------------------------------------------------

fn shrinker_consider_workload(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("shrinker_consider");

    for n_nodes in [4, 16, 64] {
        // Build n "ChoiceNode"s: each has (min, max, shrink_towards, value).
        let nodes: Vec<(IBig, IBig, IBig, IBig)> = (0..n_nodes)
            .map(|_| {
                let class = if rand_v08::Rng::gen::<bool>(&mut rng) {
                    ValueClass::OneWord
                } else {
                    ValueClass::TwoWord
                };
                let a = sample_ibig(class, &mut rng);
                let b = sample_ibig(class, &mut rng);
                let (min, max) = if a <= b { (a, b) } else { (b, a) };
                let towards = IBig::from(0);
                let value = sample_ibig(class, &mut rng);
                (min, max, towards, value)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(n_nodes),
            &nodes,
            |b, nodes| {
                b.iter(|| {
                    // Phase 1: clone all nodes (dominates at 13 % of Ir).
                    let cloned: Vec<_> = nodes
                        .iter()
                        .map(|(min, max, towards, value)| {
                            (min.clone(), max.clone(), towards.clone(), value.clone())
                        })
                        .collect();

                    // Phase 2: compute sort_key for each: sub + magnitude.
                    let sort_keys: Vec<(UBig, bool)> = cloned
                        .iter()
                        .map(|(_min, _max, towards, value)| {
                            let target = towards.clone();
                            let distance = (black_box(value) - &target).unsigned_abs();
                            let below = *value < target;
                            (distance, below)
                        })
                        .collect();

                    // Phase 3: lexicographic comparison of sort key sequences.
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

// ---------------------------------------------------------------------------
// 10. from_index binary search — IntegerChoice::from_index does a binary
//     search with BigUint arithmetic: mid = lo + ((hi - lo) >> 1), then
//     min(mid, above) + min(mid, below) comparisons.
// ---------------------------------------------------------------------------

fn ubig_binary_search_step(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_binary_search_step");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let triples: Vec<(UBig, UBig, UBig)> = (0..32)
            .map(|_| {
                let a = sample_ubig(class, &mut rng);
                let b = sample_ubig(class, &mut rng);
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let above = sample_ubig(class, &mut rng);
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
                    let mid = lo + &((hi - lo) >> 1usize);
                    let total = std::cmp::min(&mid, black_box(above))
                        + std::cmp::min(&mid, black_box(above));
                    (mid, total)
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 11. HashMap<IBig, _> insert+lookup workload — IBig is sometimes used as a
//     deterministic-id key in shrinker-adjacent data structures. Exercises
//     IBig::Hash + IBig::Eq on inline values.
// ---------------------------------------------------------------------------

fn ibig_hashmap_keys(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ibig_hashmap_keys");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let keys: Vec<IBig> = (0..128).map(|_| sample_ibig(class, &mut rng)).collect();
        // Pre-populate the map.
        let mut map: HashMap<IBig, u32> = HashMap::with_capacity(keys.len());
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
// 12. from_index full binary search — the top dashu bottleneck at ~12.6% of
//     total Ir (2.83% repr + 5.7% UBig::add + 4.1% UBig::sub). Simulates
//     IntegerChoice::from_index over a full i128-range choice: binary search
//     with mid = lo + ((hi - lo) >> 1), total = min(mid, above) + min(mid, below),
//     repeated ~128 iterations for i128::MIN..i128::MAX.
// ---------------------------------------------------------------------------

fn from_index_full_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_index_full_search");

    // i128 range: above = i128::MAX, below = i128::MIN.abs() = i128::MAX + 1
    // This is the common case for IntegerChoice{min: i128::MIN+1, max: i128::MAX, shrink_towards: 0}
    let above = UBig::from(i128::MAX as u128);
    let below = UBig::from(i128::MAX as u128 + 1);

    for target_frac in [0.0f64, 0.25, 0.5, 0.75, 1.0] {
        let target_idx = {
            let max_idx = &above + &below;
            let frac_bits = (target_frac * 1000.0) as u128;
            &max_idx * UBig::from(frac_bits) / UBig::from(1000u32)
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("frac_{:.0}pct", target_frac * 100.0)),
            &target_idx,
            |b, idx| {
                b.iter(|| {
                    let one = UBig::from(1u32);
                    let mut lo = one.clone();
                    let mut hi = std::cmp::max(&above, &below).clone();
                    while lo < hi {
                        let mid = &lo + &((&hi - &lo) >> 1usize);
                        let total = std::cmp::min(&mid, &above) + std::cmp::min(&mid, &below);
                        if total >= *black_box(idx) {
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
// 13. By-ref UBig add/sub — the from_index loop operates on &UBig references,
//     not owned values. The existing benches cover owned UBig + UBig; this
//     covers the &UBig + &UBig path which is 5.7% of Ir.
// ---------------------------------------------------------------------------

fn ubig_ref_add_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_ref_add");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
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

fn ubig_ref_sub_by_class(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_ref_sub");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord, ValueClass::JustOverInline] {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| {
                let a = sample_ubig(class, &mut rng);
                let b = sample_ubig(class, &mut rng);
                // Ensure a >= b so subtraction doesn't panic.
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
                    black_box(a) - black_box(c)
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 14. Nasty-value sort — biased_i128_sample builds a Vec of ~258 boundary
//     IBig values (0, ±1, ±2^k for k in 0..=128, min, max), dedup+sorts them.
//     quicksort on IBig is 0.65% of Ir, exercising Repr::cmp.
// ---------------------------------------------------------------------------

fn ibig_boundary_sort(c: &mut Criterion) {
    let mut rng = seeded_rng();
    c.bench_function("ibig_boundary_sort", |b| {
        let min = IBig::from(i128::MIN + 1);
        let max = IBig::from(i128::MAX);

        b.iter(|| {
            let mut values = vec![min.clone(), max.clone(), IBig::from(0)];
            for sign in [1i128, -1] {
                for exp in 0..=128u32 {
                    let v = IBig::from(sign) * IBig::from(1u128 << exp.min(127));
                    values.push(v);
                }
            }
            values.push(IBig::from(rand_v08::Rng::gen_range(&mut rng, -10i64..10)));
            values.sort();
            values.dedup();
            black_box(values.len())
        })
    });
}

// ---------------------------------------------------------------------------
// 15. UBig::min — used heavily in from_index: std::cmp::min(&mid, &above).
//     Each binary search iteration does 2× min (which is Ord::cmp + branch).
//     This micro-bench isolates UBig comparison cost on inline values.
// ---------------------------------------------------------------------------

fn ubig_min_inline(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("ubig_min");
    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        let pairs: Vec<(UBig, UBig)> = (0..32)
            .map(|_| (sample_ubig(class, &mut rng), sample_ubig(class, &mut rng)))
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
);

criterion_main!(benches);
