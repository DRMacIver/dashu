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

// ---------------------------------------------------------------------------
// 16. IntegerChoice::to_index — the *forward* index lookup. Sampled the
//     hegel-rust test suite (cargo test --features native): not in the
//     headline top-30 dashu-touchers but exercises the same
//     sub/magnitude/min/add shape as `from_index_full_search`, and is the
//     direct inverse so any improvement to `from_index`'s building blocks
//     should land here too.
//
//     The implementation (`hegel::native::core::choices::IntegerChoice::to_index`)
//     is:
//
//       above = (max - s).magnitude()
//       below = (s - min).magnitude()
//       d_abs = (value - s).magnitude()
//       d_minus_one = d_abs - 1
//       count = min(d_minus_one, above) + min(d_minus_one, below)
//       (+ 1 or 2 depending on sign / d_abs vs above)
//
//     This bench drives the body for a fixed (min, s, max) over a Vec of
//     pre-sampled `value`s, capturing the per-value cost without paying for
//     the dispatch into `IntegerChoice` itself.
// ---------------------------------------------------------------------------

fn integer_choice_to_index(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("integer_choice_to_index");

    // Two ranges: a small i128-bracket (the common case, matches the
    // `from_index_full_search` setup) and a heap-only range, so the bench
    // reflects both the inline and just-over-inline paths.
    let scenarios: [(&str, IBig, IBig, IBig); 2] = [
        (
            "i128_range",
            IBig::from(i128::MIN + 1),
            IBig::from(0),
            IBig::from(i128::MAX),
        ),
        (
            "heap_range",
            IBig::from(0),
            IBig::from(0),
            IBig::from(1u128) << 200,
        ),
    ];

    for (label, min_v, s, max_v) in scenarios.iter() {
        // Pre-sample 32 in-range values. Mix nasty pool with random draws
        // so we don't end up only exercising one branch (`d_abs <= above`).
        let values: Vec<IBig> = (0..32)
            .map(|i| {
                let class = match i % 4 {
                    0 => ValueClass::OneWord,
                    1 => ValueClass::TwoWord,
                    _ => ValueClass::OneWord,
                };
                let mag = sample_ibig(class, &mut rng);
                // Clamp into range so to_index doesn't have to reject.
                let v = if &mag > max_v {
                    max_v.clone()
                } else if &mag < min_v {
                    min_v.clone()
                } else {
                    mag
                };
                v
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(min_v.clone(), s.clone(), max_v.clone(), values),
            |b, (min_v, s, max_v, values)| {
                let mut i = 0usize;
                let one = UBig::from(1u32);
                b.iter(|| {
                    let v = &values[i & 31];
                    i = i.wrapping_add(1);
                    // Body of IntegerChoice::to_index, inlined.
                    if v == s {
                        UBig::from(0u32)
                    } else {
                        let above = (max_v - s).unsigned_abs();
                        let below = (s - min_v).unsigned_abs();
                        let d_abs = (v - s).unsigned_abs();
                        let d_minus_one = &d_abs - &one;
                        let mut count =
                            std::cmp::min(&d_minus_one, &above) + std::cmp::min(&d_minus_one, &below);
                        if v > s {
                            return count + &one;
                        }
                        if d_abs <= above {
                            count += UBig::from(1u32);
                        }
                        count + UBig::from(1u32)
                    }
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 17. NodesSortKey::cmp — lazy lex compare of two ChoiceNode sequences.
//
//     Hegel's shrinker compares pre/post candidate sequences via
//     `NodesSortKey::cmp`, which walks both sequences in lockstep and
//     computes per-node sort keys on the fly. For Integer choice nodes the
//     per-node key is `(value - shrink_towards).magnitude(), value < shrink_towards`
//     (an allocated `UBig` + a bool). Sampled at ~0.25 % inclusive of the
//     hegel-rust test suite (and `sort_key_ref` accounts for another
//     ~0.18 %).
//
//     The lazy variant is meaningfully different from
//     `shrinker_consider_workload` (which eagerly materialises all sort keys
//     into a `Vec`): when the sequences differ early, the lazy form does far
//     less work, and the per-iteration allocation cost is what the real
//     shrinker pays. Two parameterisations:
//
//     * `same_prefix` — sequences agree for the first half, differ in the
//       middle. Exercises the typical "small change to a long shrunk
//       sequence" path.
//     * `differ_at_zero` — sequences differ at position 0. Tests the early-
//       exit fast path where we only allocate two sort keys.
// ---------------------------------------------------------------------------

fn nodes_sort_key_lex_cmp(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("nodes_sort_key_lex_cmp");

    // Each "node" is a (value, shrink_towards) pair; the sort key is
    // ((value - shrink_towards).magnitude(), value < shrink_towards). This
    // matches `ChoiceNode::sort_key_ref` for the Integer variant.
    type Node = (IBig, IBig);

    fn sort_key(node: &Node) -> (UBig, bool) {
        let (value, target) = node;
        ((value - target).unsigned_abs(), value < target)
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
        // Scenario A: sequences share the first half, differ in the middle.
        let a: Vec<Node> = (0..n_nodes)
            .map(|_| (sample_ibig(ValueClass::OneWord, &mut rng), IBig::from(0)))
            .collect();
        let mut b = a.clone();
        let mid = n_nodes / 2;
        b[mid].0 = &b[mid].0 + IBig::from(1);

        group.bench_with_input(
            BenchmarkId::new("same_prefix", n_nodes),
            &(a, b),
            |bn, (a, b)| {
                bn.iter(|| lex_cmp(black_box(a), black_box(b)));
            },
        );

        // Scenario B: differ at index 0 — cmp returns after one pair of
        // sort_key allocations.
        let a: Vec<Node> = (0..n_nodes)
            .map(|_| (sample_ibig(ValueClass::TwoWord, &mut rng), IBig::from(0)))
            .collect();
        let mut b = a.clone();
        b[0].0 = &b[0].0 + IBig::from(1);
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

// ---------------------------------------------------------------------------
// 18. shrinker descent step — the inner body of `find_integer` used by
//     `binary_search_integer_towards_zero`. The shrinker tries candidates
//     `base - BigInt::from(2 * n as u64)` for n = 1, 2, 4, 8, ... until the
//     predicate fails, then binary-searches the bracket.
//
//     The dashu-touching part is the candidate construction + range
//     validation: `IBig::from(small_const)`, `&base - that`, `cand >= min &&
//     cand <= max`. That's a sub + two cmps per probe. The real shrinker
//     pays ~0.20 % of total Ir on this loop (`binary_search_integer_towards_zero`
//     inclusive in the test suite profile).
//
//     The bench drives a fixed step sequence so the cost reflects the
//     dashu-side per-probe overhead, not the test harness's branching.
// ---------------------------------------------------------------------------

fn shrinker_descent_subtract(c: &mut Criterion) {
    let mut rng = seeded_rng();
    let mut group = c.benchmark_group("shrinker_descent_subtract");

    for &class in &[ValueClass::OneWord, ValueClass::TwoWord] {
        // 32 distinct (base, min, max) triples so the bench loop sees varied
        // inputs; magnitudes match the inline workload the shrinker actually
        // touches in tests.
        let triples: Vec<(IBig, IBig, IBig)> = (0..32)
            .map(|_| {
                let base = sample_ibig(class, &mut rng);
                let min = &base - IBig::from(1024i64);
                let max = &base + IBig::from(1024i64);
                (base, min, max)
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(class.label()),
            &triples,
            |b, t| {
                let mut i = 0usize;
                // Step sequence matches the exponential probe in find_integer:
                // 1, 2, 3, 4 then geometric (8, 16, 32, ...).
                const STEPS: [u64; 9] = [1, 2, 3, 4, 8, 16, 32, 64, 128];
                b.iter(|| {
                    let (base, min, max) = &t[i & 31];
                    i = i.wrapping_add(1);
                    let mut valid_count = 0u32;
                    for n in STEPS {
                        // `&base - BigInt::from(2 * n)` is the
                        // shrink-by-multiples-of-2 probe; the linear-1 probe
                        // is `&base - BigInt::from(n)`.
                        let cand = base - IBig::from(2u64 * n);
                        if &cand >= black_box(min) && &cand <= black_box(max) {
                            valid_count += 1;
                        }
                        let cand = base - IBig::from(n);
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
