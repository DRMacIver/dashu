# Initial profile of dashu-int workloads (2026-05-30)

First-pass profiling of `dashu-int` against the new `workload` benchmarks,
done before any optimisation work. Goal: identify the biggest hot spots
on small-integer / mixed-width workloads so we can sequence the
optimisation backlog by expected payoff.

## Method

Profile target lives at `integer/examples/profile_workload.rs`. It runs
three scenarios from `benches/workload.rs` (`sum`, `str`, `mix`) in a
tight time-boxed loop. Each scenario has a `-small` variant that
restricts the input distribution to values ≤ 128 bits (i.e. always
inline in `Repr`); this exposes the small-integer hot path that the
default mixed distribution buries under rare large-value work.

Build / run:

```
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
    cargo build --release -p dashu-int --example profile_workload --features rand
samply record --save-only --no-open --unstable-presymbolicate \
    -o /tmp/prof_sum.json.gz \
    -- ./target/release/examples/profile_workload sum 10
```

Scenarios: `sum | sum-small | str | mix | mix-small`. The second arg is
the recording duration in seconds (≥ 10 s gives ≥ 10 k samples at
samply's default 1 kHz rate).

Symbolicated flat profiles are extracted by `/tmp/flat_profile.py` (a
throwaway helper, not checked in) which reads samply's `.syms.json`
sidecar, resolves raw RVAs through each module's `symbol_table`, and
prints top-by-self and top-by-inclusive. Re-run after any optimisation
to compare.

Macros: `samply record` on macOS uses xpc sampling, doesn't require
sudo, and produces a profile and a sidecar `.syms.json` next to it.

Caveats:

* Single-machine, single-run measurements. Numbers are within a few
  percent of each other across re-runs, but treat differences below
  ~2 pp as noise.
* `profile_workload::main` collects a large self-time fraction (23 % to
  46 % depending on scenario) because LLVM inlines the inner add/sub
  kernel into the per-step loop body. So that bucket is "small-int
  arithmetic plus dispatch", not "Rust loop overhead".
* Compiled with `debug = line-tables-only` for symbolication. CPU
  numbers should match a stripped release build, but absolute timings
  in the bench output do not.

## Scenarios at a glance

| Scenario     | What it exercises                                          | Inputs                              |
| ------------ | ---------------------------------------------------------- | ----------------------------------- |
| `sum`        | Running sum + bound check (targeting-loop shape)           | Mixed distribution, 1 % Large       |
| `sum-small`  | Same, but every value ≤ 128 bits                           | Only `OneWord` + `TwoWord`          |
| `str`        | Decimal-string round-trip (HegelValue protocol shape)      | Mixed distribution                  |
| `mix`        | Scripted `+ − × << & ^` over 4 live registers              | Mixed distribution                  |
| `mix-small`  | Same scripted ops, no heap values                          | Only `OneWord` + `TwoWord`          |

The mixed distribution is the same one the `workload` bench uses:
roughly 5 % zero / 60 % one-word / 25 % two-word / 7 % just-over-inline
/ 2 % mid (1024-bit) / 1 % large (100 k-bit). The 1 % Large is enough
to dominate any workload that touches multiplication.

## Headline flat profiles (top by self time)

### `sum` (full distribution)

```
self%   incl%  function
33.47   33.47  add::add_same_len_in_place
22.85   99.99  profile_workload::main
19.01   19.01  add::sub_in_place_with_sign
 6.26    6.26  add::add_dword_in_place
 5.27    5.27  Repr::from_buffer
 5.23    5.23  add::sub_dword_in_place
 2.89   46.77  Add<TypedRepr> for TypedReprRef::add
 2.58    2.58  Repr::Drop::drop
 0.92    9.10  add_ops::add_large_dword
 0.50   34.73  add_ops::add_large
 0.37    0.37  _platform_memmove
```

### `sum-small` (≤ 128-bit only — the closest analogue to a hegel
targeting/score loop)

```
self%   incl%  function
45.83   99.83  profile_workload::main
14.51   14.51  add::add_dword_in_place
12.48   12.48  add::sub_dword_in_place
 9.00    9.00  Repr::from_buffer
 7.48   29.31  Add<TypedRepr> for TypedReprRef::add
 5.78    5.78  Repr::Drop::drop
 1.97   20.57  add_ops::add_large_dword
 0.61    0.61  cmp::cmp_in_place
 0.28    0.28  _platform_memset
 0.25    1.08  Buffer::allocate_exact
```

### `mix` (full distribution, all ops)

```
self%   incl%  function
25.11   25.11  mul::simple::add_signed_mul_chunk
22.72   22.72  _platform_memmove
16.06   16.06  add::add_same_len_in_place
10.37   10.37  add::sub_in_place_with_sign
 7.49    7.53  mul_ops::mul_large_dword
 4.72   29.21  mul::karatsuba::add_signed_mul_same_len
 3.53   31.47  mul::toom_3::add_signed_mul_same_len
 2.78    4.66  shift_ops::shl_large_ref
 2.30    2.30  add::add_signed_in_place
 1.06    1.06  div::div_by_word_in_place
 0.84    0.92  bits::bitxor_large
```

### `mix-small` (diverse ops, ≤ 128 bits only)

```
self%   incl%  function
17.40   17.40  _platform_memmove
16.99   16.99  add::add_same_len_in_place
10.61   10.82  mul_ops::mul_large_dword
 8.17    8.17  add::sub_in_place_with_sign
 6.03   99.68  profile_workload::main
 4.46    8.78  shift_ops::shl_large_ref
 2.52    2.52  Repr::from_buffer
 2.39    2.39  mach_absolute_time
 2.34    2.34  _platform_memset
 1.44    1.66  bits::bitxor_large
 1.14    1.14  Repr::Drop::drop
```

## What stands out

### Allocation-shaped overhead on the small-int Add path

`sum-small` is the canonical "many cheap adds against a heap-resident
running accumulator" loop. `Repr::from_buffer` (9.0 %) + `Repr::Drop`
(5.78 %) = **14.78 % of CPU is per-step allocator bookkeeping** that
the operation itself doesn't need.

The mechanism is in `integer/src/add_ops.rs:84–116` and
`integer/src/repr.rs:317`. Each `&UBig + &UBig` (and `IBig`)
dispatches through the `TypedRepr` enum into one of `add_large_dword`,
`add_large`, etc., and every one of them ends with
`Repr::from_buffer(buffer)` which unconditionally runs:

1. `Buffer::pop_zeros()` — walks the buffer top-down looking for
   trailing zeros.
2. `Buffer::shrink_to_fit()` — checks the capacity-vs-length growth
   bound and reallocates if exceeded.

For an accumulator that's been heap-resident for many iterations and
will remain heap-resident, that work is pure overhead — the result is
always large enough that no inline / shrink decision is possible.

The `Drop::drop` cost (`Repr` deallocating its buffer, defined at
`integer/src/repr.rs:542`) is harder to pin to a single call site
without inclusive-call-graph data, but the most likely sources are:

* `shrink_to_fit` deciding to realloc (rare on this workload, but
  measurable).
* Argument or intermediate `Buffer` allocations being dropped at the
  end of each `+`. The `+=` path normally avoids these via
  `core::mem::take` (see `impl_binop_assign_by_taking` in
  `integer/src/helper_macros.rs:317`), but it's worth confirming by
  call-graph the next time we profile.

**Estimated payoff** of a specialised `AddAssign<&IBig> for IBig`
(and the `<i64>` / `<u64>` variants) that bypasses `from_buffer` when
the operation provably can't shrink the magnitude class: 8–12 % on
`sum-small`. Lower on the mixed distribution, because the rare Large
values dominate that timing.

### The "both inline" Add path is dispatched, not specialised

`add_dword: DoubleWord → DoubleWord → Repr` (`add_ops.rs:141`) exists
but is only reached when both operands arrive as
`TypedRepr::Small` / `TypedReprRef::RefSmall`. Reaching that match arm
requires walking the full dispatch chain:

```
Add<&IBig> for IBig          (helper_macros.rs:241)
  → into_sign_repr / as_sign_repr        (per side)
  → impl_ibig_add!(...) match on sign    (add_ops.rs:11)
  → Add<TypedReprRef> for TypedRepr      (add_ops.rs:112)
  → swap to commutative form
  → Add<TypedRepr> for TypedReprRef      (add_ops.rs:99)
  → match on (RefSmall, Small)
  → add_dword
```

That chain shows up at **7.48 % self / 29.3 % inclusive** in
`sum-small` (`Add<TypedRepr> for TypedReprRef::add` line). Each step
is a tiny match-and-branch, but at the inner-loop hot path the
inlining isn't perfect and the cumulative dispatch cost is large.

A direct `impl AddAssign<&IBig> for IBig` that peeks at the two
`Repr::capacity` words and short-circuits the inline + inline → inline
case (with a fall-back to the current path) would skip almost all of
this. Same shape works for `Sub`, `BitAnd`, `BitOr`, `BitXor`.

### `_platform_memmove` at 17 % in `mix-small` is suspicious

For a workload where every input is ≤ 128 bits, there is no
fundamental reason for `memmove` to be a top-5 cost. The two prime
suspects:

1. **`Buffer::from(&[Word])` in the borrowed-Add path.** Looking at
   `Add<TypedReprRef<'r>> for TypedReprRef<'l>` in `add_ops.rs:80`:
   ```rust
   (RefLarge(words0), RefSmall(dword1)) =>
       add_large_dword(words0.into(), dword1),
   ```
   `words0.into()` invokes `Buffer::from(&[Word])`, which allocates a
   fresh buffer and `memcpy`s the LHS slice into it. The `+=` form
   avoids this via `take`, but the `mix` scenario uses `&r0 + &r2`,
   `&r0 ^ &r1`, etc., which all take this borrowed/borrowed path.
2. **`shrink_to_fit` calling `realloc`.** `realloc` on a shrinking
   allocation usually translates to alloc-new + memmove-old + free-old.
   This shouldn't fire often because of the `max_compact_capacity`
   slack rule (`buffer.rs:68`), but every fire is a full memmove.

Both are addressable. The borrowed/borrowed Add could allocate the
result buffer at the correct size up front and write into it directly,
without the intermediate copy. `shrink_to_fit` could be skipped when
we know the result is heap-sized.

### `add_dword_in_place` and `sub_dword_in_place` look tight already

In `sum-small`, these two account for 14.51 % + 12.48 % = 27 % self
time. The source (`integer/src/add.rs:55–93`) is straightforward —
two `overflowing_add` / `overflowing_sub` calls and a carry-out
propagation via `add_one_in_place` / `sub_one_in_place`. The fast
path for "carry stopped at word 2" already short-circuits with `&&`.

So this is not a "the inner kernel is slow" problem. It's a "we run
the inner kernel a lot, and the surrounding plumbing (dispatch +
finalisation) costs almost as much as the kernel" problem. Optimise
the plumbing.

### Mul path in `mix` is the obvious large-input bottleneck

`add_signed_mul_chunk` (25 %) + `memmove` (23 %) + the karatsuba /
toom-3 wrappers tell the whole story for the mixed distribution. The
1 % Large values do ~98 % of the multiplication work, and we spend a
quarter of the workload just shovelling words through the inner mul
loop. Speeding up the small-int path won't move this needle; the path
to faster `mix` on the full distribution runs through faster
multiplication on 100 k-bit values, which is a separate (much larger)
project.

For the hegel use case specifically, this is probably not the priority
— the value distribution in real hegel runs is much more skewed toward
small than this synthetic mix.

## Ranked recommendations for the next change

Each item lists the operation, the file(s) likely to change, the
expected payoff on `sum-small` (which is the closest analogue to the
hegel hot path), and a sketch of correctness risk.

1. **Specialised `AddAssign<&IBig> for IBig` (and `UBig`, `i64`,
   `u64`, `i128`, `u128`) that bypasses `Repr::from_buffer` when the
   result is provably still heap-sized.**
   * Files: `integer/src/add_ops.rs`, with a new in-place helper in
     `integer/src/repr.rs` that does `pop_zeros` only (no
     `shrink_to_fit`).
   * Estimated payoff: 8 – 12 % on `sum-small`; smaller on mixed.
   * Risk: low. The stateful equivalence tests cover sign / carry
     behaviour and will catch any divergence.
   * Side benefit: `SubAssign`, `BitAndAssign`, etc. are the same
     shape — once the pattern is in place, sliding the other ops
     into it is mechanical.

2. **Inline + inline Add fast path, directly from `IBig::add_assign`,
   without going through `TypedRepr` dispatch.**
   * Files: `integer/src/add_ops.rs`, possibly `integer/src/repr.rs`
     to expose the capacity / inline view ergonomically.
   * Estimated payoff: 5 – 10 % on `sum-small`. Bigger on a workload
     where the accumulator stays inline across resets.
   * Risk: low. Falls back to the current path when capacities don't
     match the inline case.

3. **Trace and fix the `mix-small` memmove cost.**
   * First step is more profiling: re-run `mix-small` and look at the
     inclusive call graph for `_platform_memmove`. Cheapest way is to
     load the existing samply profile in the Firefox profiler — the
     call tree there will say which dashu function is the parent.
   * Likely fix: change `Buffer::from(&[Word])` callers in the
     borrowed/borrowed Add (and Sub) impls to allocate at the result
     size and write directly, instead of allocating a copy of the
     longer operand and mutating it.
   * Estimated payoff: hard to predict without the call graph — could
     be 5 – 15 % on `mix-small`, much less on other scenarios.
   * Risk: medium. Touches an allocation-shape change in a path that
     several Add / Sub / bitwise impls share.

4. **(Out of scope for now)** Faster multiplication on Large values.
   The path through `add_signed_mul_chunk` and the karatsuba / toom-3
   wrappers is where the `mix` and `str` scenarios spend most of
   their cycles, but improving it requires deep work in the
   multiplication module and isn't aligned with the hegel-rust
   small-int optimisation goal. Note for later.

## Suggested workflow for follow-up

* Land items 1 and 2 in a single PR with `--save-baseline pre-opt`
  numbers attached. The stateful tests + the existing unit tests
  should catch regressions; the bench delta tells us if the
  optimisation moved the right needle.
* Capture before/after flat profiles using the same procedure as
  above and append the diff to this notes directory.
* Only then look at item 3 — if items 1 + 2 reshape the dispatch
  layer, the memmove story may change.

## Raw artefacts

Per-scenario raw profiles (not committed) live under `/tmp/` after
running the recording script:

```
/tmp/prof_sum.json.gz       /tmp/prof_sum.json.syms.json
/tmp/prof_sum_small.json.gz /tmp/prof_sum_small.json.syms.json
/tmp/prof_str.json.gz       /tmp/prof_str.json.syms.json
/tmp/prof_mix.json.gz       /tmp/prof_mix.json.syms.json
/tmp/prof_mix_small.json.gz /tmp/prof_mix_small.json.syms.json
```

The Gecko `.json.gz` files load directly in
<https://profiler.firefox.com> (drop file onto the page; the
`.syms.json` sidecar must sit beside it for symbolication).
