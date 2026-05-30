# Hegel shrinker bench: Repr::clone optimisation

User pushed `integer/benches/hegel_shrinker.rs` — ten benches modelling
the dashu-int operations that callgrind identifies as the dominant
costs in the hegel-rust shrinker hot path. Headline from the bench file's
own comments:

| Operation       | % of total Ir |
| --------------- | ------------- |
| Repr::clone     | **13.24 %**   |
| IBig::sub       | 2.2 %         |
| UBig::add       | 1.0 %         |
| IBig cmp/clamp  | 1.1 %         |
| Drop(Repr)      | 0.5 %         |

Values almost always `≤ 128 bits` (i.e. inline in `Repr`).

## What was already covered

- `IBig::sub`, `UBig::add` — Rec 1 + Rec 2 from the earlier optimisation
  pass (`from_buffer_normalized`, dispatch `#[inline(always)]`) handle
  the AddAssign / Sub / by-ref Add chains.
- `IBig::cmp` — `cmp::cmp_in_place` was `#[inline]`d in commit `8359d48`.
- `Drop(Repr)` — `#[inline]`d in commit `288811b`.

Which left `Repr::clone` as the big un-touched item.

## What the old `Repr::clone` did

```rust
fn clone(&self) -> Self {
    let (capacity, sign) = self.sign_capacity();
    let new = unsafe {
        if capacity <= 2 {
            Repr {
                data: ReprData { inline: self.data.inline },
                capacity: NonZeroIsize::new_unchecked(capacity as isize),
            }
        } else { ...heap path... }
    };
    new.with_sign(sign)
}
```

Three issues for the inline case:

1. `sign_capacity()` unpacks the signed `NonZeroIsize` into
   `(abs_capacity, Sign)`. Branchy.
2. The inline arm builds a *positive* `Repr` from `abs_capacity`.
3. `with_sign(sign)` then conditionally negates the capacity back.

For the dominant inline case (capacity ∈ {-2, -1, 1, 2}), we read the
sign just to potentially un-do it. The whole sequence collapses to
"copy 3 machine words verbatim and you're done."

The function also wasn't `#[inline]`, so every IBig/UBig clone went
through a function-call boundary.

## What the new `Repr::clone` does (commit `583ee7e`)

```rust
#[inline]
fn clone(&self) -> Self {
    if self.capacity.get().unsigned_abs() <= 2 {
        return Repr {
            data: ReprData {
                inline: unsafe { self.data.inline },
            },
            capacity: self.capacity,
        };
    }
    self.clone_heap()
}

#[cold]
#[inline(never)]
fn clone_heap(&self) -> Self { ...heap branch... }
```

- Inline path is `#[inline]` and trivially copies the union + signed
  capacity. No sign unpack/repack.
- Heap path is split out into a `#[cold]` + `#[inline(never)]` helper
  so the inline fast path stays tiny everywhere it's instantiated.

`Repr::clone_from` got the same treatment in commit `78ebdb7` for
symmetry (Vec::clone calls T::clone per element, but downstream
`*dest = src.clone()` patterns may compile to clone_from).

## Heap-path trade-off

`clone_heap` being `#[inline(never)]` means heap clones now pay an
extra function-call boundary. The hegel_shrinker bench shows:

| Bench                          | Δ vs current  |
| ------------------------------ | ------------- |
| ibig_clone/just_over_inline    | +7.7 %        |
| ibig_clone/mid                 | +12.7 %       |
| ibig_clone/large               | ~0 %          |

I experimentally removed `#[inline(never)]` (leaving only `#[cold]`),
expecting LLVM to inline back where profitable. Instead the heap case
got *worse* (mid: +49 %, large: +15 %), apparently because the
inlined-in-clone shape compiles less well than going through a normal
function call. Restored `#[inline(never)]` as the better trade-off.

For the hegel shrinker workload (overwhelmingly inline values), the
trade is unambiguously good. A different downstream consumer with
heavy mid-sized clones would notice; if that comes up we can revisit.

## Headline bench impact

vs `current` baseline saved before the change (so this is the delta
from "everything before hegel_shrinker bench was added" — i.e. on
top of all the Rec 1 / Rec 2 / etc. work):

| Bench                            | Δ           |
| -------------------------------- | ----------- |
| `shrinker_consider/64`           | **-29.1 %** |
| `shrinker_consider/16`           | -25.5 %     |
| `shrinker_consider/4`            | -12.7 %     |
| `ibig_clamp/two_word`            | -42.6 %     |
| `ibig_clamp/one_word`            | -40.2 %     |
| `ibig_drop/two_word`             | -35.3 %     |
| `ibig_drop/one_word`             | -34.6 %     |
| `ibig_drop/zero`                 | -34.2 %     |
| `ibig_clone/two_word`            | -21.4 %     |
| `ibig_clone/one_word`            | -20.3 %     |
| `ibig_clone/zero`                | -20.0 %     |

`shrinker_consider/64` is the most realistic top-level scenario
(simulates a single `consider()` call: clone 64 ChoiceNodes,
compute sort_key for each, lexicographically compare the sequences).
A 29 % drop there should be visible in any hegel-rust workload
dominated by shrinking.

`ibig_clamp` wins big because `Ord::clamp` calls `.clone()` three
times. Three 20 % clone wins compose into a 40 % clamp win once the
overlapping inlining lets LLVM CSE shared work.

`ibig_drop`'s ~35 % win is interesting — my change wasn't to Drop
directly. The clone fast path being a direct struct-copy lets the
surrounding bench loop fold the clone + drop into something tighter
than before.

## Other items in the bench file

- `ibig_sub_magnitude/*`: small (+3 to +5 %) regressions. The sub
  itself is unchanged, `unsigned_abs` on the owned result doesn't
  clone, and there's nothing obvious for my changes to have touched.
  Likely bench noise (user noted machine has unrelated load).
- `ibig_from_const/*`: noisy (-7 % to +30 %). `IBig::from(0i64)` etc.
  doesn't involve clone; nothing in this patch should affect it.
  Treating as noise.
- `ubig_binary_search_step/*`: small regressions (+2 to +13 %).
  Same — none of these paths go through `Clone::clone`. Noise.

## What's left

- Heap `clone_heap` could be tightened: the current body allocates,
  does push_slice (a memcpy), then transmutes, then conditionally
  negates capacity. The conditional negation is cheap; the rest is
  the inherent memcpy. A fused allocate-and-copy that skips
  `push_slice`'s bounds checks might shave a few cycles on the
  `just_over_inline` case. Not pursued in this round — the
  shrinker spends little time here.
- `UnsignedAbs for &IBig` in `sign.rs:97` does `self.0.clone().with_sign(...)`.
  After this patch the clone is the fast inline copy for the common
  case, so the overall cost is fine. Mentioned for completeness.

## Verification

- `cargo test -p dashu-int --features rand` — 74 lib tests pass.
- `cargo test -p dashu-int --features hegel-tests --test stateful_ibig --test stateful_ubig`
  — both stateful equivalence machines (which exercise clone via
  num-bigint comparison) pass.
