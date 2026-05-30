//! Shared helpers for the `small_int` and `workload` benchmarks.
//!
//! Included into each bench via `#[path = "common/mod.rs"] mod common;`.
//! Lives in a subdirectory so cargo's bench autodiscovery ignores it.

#![allow(dead_code)]

use dashu_int::{IBig, UBig};
use rand_v08::prelude::*;
use rand_v08::rngs::StdRng;

/// Coarse value-magnitude classes used to drive the bench parameter sweeps.
///
/// Intent is to exercise each interior path of `Repr` rather than only the
/// large-buffer paths covered by the existing `primitive.rs` benches.
#[derive(Clone, Copy, Debug)]
pub enum ValueClass {
    /// Exactly zero. Common in real workloads (initial accumulators, defaults).
    Zero,
    /// Fits in a single `Word` (≤ 64 bits on 64-bit targets). Single-word inline.
    OneWord,
    /// Needs both inline words (65–128 bits). Still inline, but the upper word
    /// is meaningful.
    TwoWord,
    /// Just past the inline boundary (129–256 bits). Heap-allocated but tiny.
    JustOverInline,
    /// Medium (~1024 bits). Multi-limb but still in fast-path territory for
    /// schoolbook arithmetic.
    Mid,
    /// Large (~100k bits). Same regime as the existing `primitive` benches.
    Large,
}

impl ValueClass {
    pub const ALL: &'static [ValueClass] = &[
        ValueClass::Zero,
        ValueClass::OneWord,
        ValueClass::TwoWord,
        ValueClass::JustOverInline,
        ValueClass::Mid,
        ValueClass::Large,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ValueClass::Zero => "zero",
            ValueClass::OneWord => "one_word",
            ValueClass::TwoWord => "two_word",
            ValueClass::JustOverInline => "just_over_inline",
            ValueClass::Mid => "mid",
            ValueClass::Large => "large",
        }
    }
}

/// Sample a `UBig` from the given class.
pub fn sample_ubig<R: Rng>(class: ValueClass, rng: &mut R) -> UBig {
    match class {
        ValueClass::Zero => UBig::from(0u32),
        // Non-zero so the inline path is exercised meaningfully.
        ValueClass::OneWord => UBig::from(rng.gen::<u64>() | 1),
        ValueClass::TwoWord => {
            let lo: u64 = rng.gen();
            let hi: u64 = rng.gen::<u64>() | (1 << 63); // force the top word non-empty
            (UBig::from(hi) << 64) + UBig::from(lo)
        }
        ValueClass::JustOverInline => random_ubig(192, rng),
        ValueClass::Mid => random_ubig(1024, rng),
        ValueClass::Large => random_ubig(100_000, rng),
    }
}

/// Same shape as `sample_ubig`, but produces `IBig` and alternates sign for
/// even/odd RNG draws so negative paths get exercised.
pub fn sample_ibig<R: Rng>(class: ValueClass, rng: &mut R) -> IBig {
    let mag = IBig::from(sample_ubig(class, rng));
    if rng.gen::<bool>() {
        -mag
    } else {
        mag
    }
}

/// Helper modelled on the one in `primitive.rs`: a uniformly distributed UBig
/// of approximately `bits` bits (at least 2^(bits-1)).
pub fn random_ubig<R: Rng>(bits: usize, rng: &mut R) -> UBig {
    rng.gen_range(UBig::ONE << (bits - 1)..UBig::ONE << bits)
}

/// Draw a class from a distribution that approximates a hegel-style workload:
/// small values dominate, large values are rare. Numbers sum to 100.
pub fn mixed_class<R: Rng>(rng: &mut R) -> ValueClass {
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

pub fn seeded_rng() -> StdRng {
    StdRng::seed_from_u64(0xDA5_4_BE_4)
}
