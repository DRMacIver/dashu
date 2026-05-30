# Final summary — optimisation pass against the initial profile

Wraps up the sequence that started with
[`2026-05-30-initial-profile.md`](./2026-05-30-initial-profile.md). All
four recommendations from the original notes plus several new inline
opportunities surfaced by the post-Rec1 / post-Rec2 profiles have been
either landed or explicitly ruled out.

## Headline workload results

All numbers are vs the `pre-opt` baseline saved on commit `a8c9427`,
measured on a quiet macOS machine, criterion defaults.

| Bench                            | Δ (cumulative) |
| -------------------------------- | -------------- |
| `running_sum_and_compare_small`  | **-50 %**      |
| `running_sum_and_compare`        | **-22 %**      |
| `bounded_arithmetic_mix_small`   | -7 %           |
| `bounded_arithmetic_mix`         | -1 %           |
| `string_round_trip`              | ~0 %           |

The hegel-style hot path (`sum-small`: heap accumulator, mostly-inline
RHS, `+=`-style loop) is now 2× as fast. The mixed-distribution sum
(occasional Large RHS values) is ~1.28× faster. Workloads dominated
by either large multiplication (`mix`) or string parsing
(`string_round_trip`) are unchanged.

Selected diagnostic micro-benches (the cleanest signal for *why* the
workload improved):

| Bench                                                    | Δ           |
| -------------------------------------------------------- | ----------- |
| `ubig_add_assign_heap_acc_small_rhs/large`               | **-42 %**   |
| `ubig_add_assign_heap_acc_small_rhs/mid`                 | -28 %       |
| `ubig_add_assign_heap_acc_small_rhs/just_over_inline`    | -28 %       |
| `ibig_add_assign_i64_into_heap_acc`                      | -32 %       |
| `ibig_add_assign_i128_into_heap_acc`                     | -32 %       |
| `ubig_add_assign_u64_into_heap_acc`                      | -28 %       |
| `ubig_add_assign_u128_into_heap_acc`                     | -28 %       |
| `ibig_add_by_class/mid`                                  | -16 %       |

## What landed, in order

| Commit     | Change                                                                       | Approx delta on `sum-small` |
| ---------- | ---------------------------------------------------------------------------- | --------------------------- |
| `a8c9427`  | (Benchmark scaffolding — covered in Rec 1 notes.)                            | —                           |
| `8867b33`  | Rec 1: `from_buffer_normalized` on heap-stay-heap Add/Sub paths              | -16 %                       |
| `288811b`  | `#[inline]` on `Drop for Repr` / `Drop for Buffer`                           | -33 %                       |
| `09a7a2d`  | Rec 2: `#[inline(always)]` on Add/Sub/SubSigned dispatch impls               | -40 %                       |
| `13a6297`  | Rec 3 (partial): `#[inline(always)]` on `Buffer::from(&[Word])`              | -40 %                       |
| `8359d48`  | `#[inline]` on `cmp::cmp_in_place`                                           | -47 %                       |
| `b5547c5`  | `#[inline]` on inner add/sub kernels (`*_in_place`)                          | -50 %                       |
| `396464a`  | `#[inline]` on shift kernel + Mul dispatch `#[inline(always)]`               | -50 %                       |

The recurring pattern: a tiny `pub fn` body whose call boundary
prevented LLVM from folding the surrounding loop. The Drop and
cmp_in_place changes alone contributed half the headline win;
`from_buffer_normalized` (the Rec 1 algorithmic change) contributed
the other ~16 pp and unblocked the rest by removing per-iteration
trim/shrink work.

## Recommendations status

| # | Recommendation                                                  | Status   |
| - | --------------------------------------------------------------- | -------- |
| 1 | Skip `from_buffer` on heap-stay-heap                            | **Done** |
| 2 | Collapse the dispatch chain                                      | **Done** |
| 3 | Borrowed-Add fused alloc-and-fill                                | **Partial / ruled out** — see [rec2/rec3 notes](./2026-05-30-rec2-rec3-results.md). The cheap part (`Buffer::from #[inline(always)]`) landed; the fused-write rewrite was not pursued because the remaining cost is the inherent word-level data movement, and a real caller can already side-step it via `+=` (which Rec 1 has optimised). |
| 4 | Faster large multiplication (FFT, Toom-3 tuning, …)              | **Ruled out** for this pass. Multi-week project, orthogonal to the hegel-rust small-int goal. The `fft` module already exists in scaffolded form; a future investigation can pick it up. |

## New opportunities found and noted (not pursued in this pass)

Listed in
[`2026-05-30-rec2-rec3-results.md`](./2026-05-30-rec2-rec3-results.md):

- **`#[inline(always)]` on `forward_ibig_binop_to_repr` macro** — tried
  it, **regressed `sum-small` by ~5 pp**. The macro is shared with Mul
  / Gcd / … and the bloat hurt the i-cache more than the inlined
  dispatch helped. Reverted. Recorded in commit log of `09a7a2d` as
  a tried-and-rejected approach.
- **`#[inline]` on `sub_in_place`, `sub_same_len_in_place`,
  `sub_same_len_in_place_swap`** — tried, **regressed
  `bounded_arithmetic_mix` by ~1 pp** because the multiplication
  kernels use these in tight loops and the inlined body bloated the
  Karatsuba/Toom-3 code path. Reverted.
- **SIMD-tuned `add_same_len_in_place` / `sub_in_place_with_sign`** —
  the inner loop has a carry dependency between iterations that scalar
  Rust cannot vectorise. aarch64's `adcs` chain would help but at the
  cost of arch-specific code, which violates the user's "generically
  useful" constraint.
- **Buffer pool for hot accumulator workloads** — `Buffer::allocate_exact`
  is ~9 % inclusive on `mix-small`. A thread-local pool would amortise
  the alloc cost; sees use in `malachite`, `rug`. Bigger design change;
  warrants its own design pass.
- **`Add for &IBig` macro `#[inline(always)]`** — would help mix-small
  (still 2.69 % self in its profile) but risks regression elsewhere;
  the safe version is to add a hand-rolled non-macro `Add` impl just
  for the heap-LHS + inline-RHS case, but the win is modest and the
  code-size cost concrete.

## Final flat profiles

### `sum-small`

```
self%   incl%  function
82.41  100.00  profile_workload::main      (inlined hot path)
12.72   12.72  add_ops::repr::add_large_dword
 0.46    0.46  _platform_memset
 0.38    0.38  Repr::from_buffer
```

95 % of CPU is now in two functions: `main` (which absorbed the full
dispatch + kernels + `from_buffer_normalized`) and the
`add_large_dword` kernel (carry-propagation). At the floor for what
portable Rust can do here without writing arch-specific intrinsics.

### `mix-small`

```
self%   incl%  function
18.68   18.68  add_ops::repr::add_large
17.30   17.30  _platform_memmove
 8.99  100.00  profile_workload::run_mix
 5.68   10.75  mul_ops::mul_large_dword
 4.94    4.94  mul::mul_dword_in_place
 3.94    3.94  add::sub_same_len_in_place_swap
 3.34    3.34  mach_absolute_time           (criterion overhead)
 2.69   42.43  Add for &IBig::add
```

The `_platform_memmove` is the result-buffer memcpy on by-ref Add
of heap-resident operands. The kernels (add, mul, sub) are doing
necessary arithmetic. There is no remaining easily-removed function-call
overhead in this profile.

## Code-size impact

Tracked in [`code-size-tracking.md`](./code-size-tracking.md). Net
bloat on the bench binaries (text section):

- `workload` bench: +1.4 % (+32 KB).
- `small_int` bench: +5.3 % (+131 KB). The higher number is partly
  because the bench file has many distinct parameterised call sites
  per `ValueClass`; a single-call-site downstream consumer should be
  closer to the `workload` number.

The user has flagged the bloat as a concern; the plan is a targeted
removal pass after the optimisation work stabilises (see
`code-size-tracking.md`).

## Verification

- `cargo test -p dashu-int --features rand` — 74 lib tests + integration
  tests pass.
- `cargo test -p dashu-int --features hegel-tests --test stateful_ibig --test stateful_ubig`
  — both stateful equivalence machines pass against `num-bigint` after
  every change.
- The targeted regression tests for the bugs discovered earlier (in
  `integer/tests/shift.rs` and `integer/tests/convert.rs`) still pass.

## Repro

```
# Baseline (must be done from a tree at or before commit a8c9427):
cargo bench -p dashu-int --bench workload --features rand -- --save-baseline pre-opt
cargo bench -p dashu-int --bench small_int --features rand -- --save-baseline pre-opt

# Post-opt comparison (with this branch's tip checked out):
cargo bench -p dashu-int --bench workload --features rand -- --baseline pre-opt
cargo bench -p dashu-int --bench small_int --features rand -- --baseline pre-opt

# Profile re-run:
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
    cargo build --release -p dashu-int --example profile_workload --features rand
samply record --save-only --no-open --unstable-presymbolicate \
    -o /tmp/prof_sum_small.json.gz \
    -- ./target/release/examples/profile_workload sum-small 10
python3 /tmp/flat_profile.py /tmp/prof_sum_small.json.gz /tmp/prof_sum_small.json.syms.json
```
