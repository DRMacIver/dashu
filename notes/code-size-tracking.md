# Code-size tracking under the inlining sweep

This sequence has aggressively added `#[inline]` / `#[inline(always)]`
across dispatch chains, kernels, Drop impls, and helpers. The user has
flagged code-size bloat as a concern; the plan is to keep inlining now
for measurement clarity, then do a targeted removal pass at the end.

Measurement: text-section size of the release benchmark binaries (these
contain all of `dashu-int` plus the bench harness; the rlib alone is
hard to compare cleanly because of generic instantiation). Measured via
`size <binary>`.

## Sizes vs the pre-opt baseline

The Aug-30 6:50 / 6:51 builds are pre-opt (before any inline change in
this branch). The Aug-30 10:13 / 10:15 / 10:16 builds are post-opt
(after all the changes through commit `396464a`).

| Binary             | Pre-opt   | Post-opt  | Δ bytes  | Δ %  |
| ------------------ | --------- | --------- | -------- | ---- |
| `workload` bench   | 2,359,296 | 2,392,064 |  +32,768 | +1.4 |
| `small_int` bench  | 2,473,984 | 2,605,056 | +131,072 | +5.3 |

`small_int` has more distinct call-sites (per-`ValueClass` parameter
sweeps × many bench functions), so it picks up more duplicated inline
instantiations than `workload`. A realistic downstream consumer with
one or two call sites should be closer to the `workload` bloat number
than the `small_int` one.

## Change-by-change attribution (rough)

Per-commit deltas aren't measured directly (would require rebuilding at
each commit). The big bloat contributors, ranked by amount of inlined
code each adds:

1. **`Add` / `Sub` / `SubSigned` dispatch `#[inline(always)]`** (commit
   `09a7a2d`) — 8 enum-matching functions promoted. Each call site
   now expands the full 4-way match plus the kernel callees' setup.
2. **`Mul` dispatch `#[inline(always)]`** (commit `396464a`) — same
   shape, 4 more functions. Probably similar amount of duplication.
3. **`add::add_dword_in_place` / `sub_dword_in_place` /
   `add_one_in_place` / `sub_one_in_place` / `add_same_len_in_place` /
   `sub_in_place_with_sign` `#[inline]`** (commit `b5547c5`) — these
   are the inner loop bodies. Inlining them propagates the loop into
   every caller (which is many places across `add_ops`, `mul_ops`,
   bitwise, etc.).
4. **`from_buffer` / `from_buffer_normalized`** (Rec 1, commit
   `8867b33`) — small functions; bloat is negligible per call site.
5. **`Drop for Repr` / `Drop for Buffer`** (commit `288811b`) —
   tiny bodies, but called from every value drop. Probably the
   biggest *instantiation* multiplier even if per-site bloat is
   small.

## Plan for the targeted removal pass

Once benchmarks stabilise, revisit each `#[inline(always)]` and ask:

- Does it still win on the headline benches? Remove if not.
- Does it bloat the `small_int` or `workload` binary by more than
  ~10 KB per call site? Consider stepping back to `#[inline]`.

`#[inline(always)]` is a stronger commitment than `#[inline]` and only
some sites need it — the ones where the `TypedRepr` enum's 32-byte
payload pushes LLVM's inliner past its cost threshold. The kernel
`#[inline]`s (the `add::*_in_place` family) are likely fine because
they're below LLVM's default body-size cutoff anyway, so the
attribute is more of a documentation hint than a forcing function
for them.

## Re-measurement cadence

Quick re-measurement after any further inline change:

```
cargo build --release --bench workload --bench small_int --features rand
size target/release/deps/workload-*.* target/release/deps/small_int-*.*
```

Append the new row(s) to the table above, comparing against the
pre-opt entry, not the previous post-opt entry.
