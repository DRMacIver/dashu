# Profile-driven follow-up round

Profile-driven pass focused on what's *left* after the headline
Repr::clone + dispatch optimisations. User goal: heavily optimise
i128-fitting workloads, minimal regression on the inline/heap
boundary.

## Profiles examined

### sum-small (post-Rec1+Rec2+clone-opt)

```
self%   incl%  function
81.7   100.0   profile_workload::main      (inlined hot path)
13.4    13.4   add_large_dword             (carry-chain kernel)
 0.5     0.5   Repr::from_buffer
 0.4     0.4   _platform_memset
```

95 % in two functions: `main` (absorbed dispatch + cmp + drop + dispatch)
and `add_large_dword` (kernel). At the floor for portable Rust scalar
arithmetic.

### shrinker_consider/64 (post-Repr::clone optimisation)

```
self%   incl%  function
68.6    68.6   <Map<I,F> as Iterator>::fold  (inlined map+collect pipeline)
15.7    15.7   drop_in_place<(IBig×4)>       (per-ChoiceNode drop)
 4.6   100.0   criterion::Bencher::iter
 2.9     2.9   mach_absolute_time            (criterion timer)
 2.7     2.7   Vec::drop
```

Everything else is sub-1 %. The `map::fold` frame absorbs all inlined
work (clones, sort_key computation, comparisons) — opaque to sampling.
`drop_in_place` for the 4-IBig tuple is at the floor: 4 inline Repr drops
+ prologue/epilogue ≈ 26 instructions per tuple, which is ~3 ns per drop
at observed sample rates.

### mix-small (post-Rec2)

```
self%   incl%  function
23.0    23.0   add::add_large           (large+large kernel)
13.0    13.0   _platform_memmove        (by-ref Add memcpy)
 8.0   100.0   profile_workload::run_mix
 6.9     6.9   sub_same_len_in_place_swap
 5.1    10.0   mul_large_dword
```

Dominated by kernel work; the by-ref API for the bench's scripted
`&r0 + black_box(v)` shape is inherently O(words) memcpy. Out of
scope (see prior notes — fused alloc-and-fill was ruled out).

## Targets identified — what landed

### `Repr::Hash::hash` and `Repr::as_sign_slice` `#[inline]` (commit `646f27a`)

Both tiny functions on the IBig/UBig hash path (derived Hash delegates
to Repr) that weren't `#[inline]`. Same pattern as the earlier Drop /
cmp_in_place inlines: trivial body, large win from removing the
function-call boundary.

Validation: added a HashMap<IBig, u32> insert-prebuilt + lookup bench
(`ibig_hashmap_keys`, commit `51e6d70`). Steady-state lookup at
~19.5 ns per call across both `one_word` and `two_word` — at the floor
for HashMap bucket lookup + tiny Hash computation.

## Targets identified — explored and rejected

### `Repr::from_dword_neg` to skip `.neg()` after sub overflow

`sub_dword`'s overflow path does `Repr::from_dword(val.wrapping_neg()).neg()`,
which builds a positive Repr and then flips the sign. `.neg()` has an
is-zero check that's provably dead at this call site (`val.wrapping_neg()`
is non-zero when overflow happens).

Adding a `Repr::from_dword_neg(n)` that builds the negative Repr
directly *regressed* `ibig_sub_magnitude` by ~7 % on `one_word` and
`two_word`, with no clear win on `just_over_inline`. LLVM seems to be
eliding the is-zero check on the existing pattern better than my
hand-rolled direct construction allows. Reverted.

### `#[inline(always)]` on `add_large_dword`

Catastrophic: `running_sum_and_compare` +33 %, `running_sum_and_compare_small`
went from -50 % to -9 %. The function body (~25 instructions including
the carry-propagation loop) inlines at ~6 call sites in the dispatch
chain; cumulative bloat overwhelmed any call-site savings.

### `#[inline]` on `Repr::from_ref` / `into_buffer`

Not on shrinker / sum / mix hot paths (used in log / pow / fmt only).
Skipped.

## Results recap (against `current` baseline saved before this round)

| Bench                              | Δ          |
| ---------------------------------- | ---------- |
| `shrinker_consider/64`             | **-32.8 %** |
| `shrinker_consider/16`             | -27.8 %    |
| `shrinker_consider/4`              | -17.7 %    |
| `ibig_clamp/two_word`              | -40.1 %    |
| `ibig_clamp/one_word`              | -34.2 %    |
| `ibig_clone/two_word`              | -20.0 %    |
| `ibig_clone/one_word`              | -18.0 %    |
| `ibig_clone/zero`                  | -17.6 %    |
| `ibig_clone/large`                 | -4.6 %     |
| `ibig_clone/just_over_inline`      | +0.7 %     |
| `ibig_clone/mid`                   | +0.8 %     |
| `ubig_cmp_shrinker/*`              | -0 to -2 %  |
| `ibig_double_cmp/*`                | -0 to -3 %  |
| `ibig_shr_descent/*`               | -1 to -3 %  |
| `ubig_binary_search_step/*`        | -1 to -3 %  |
| `ibig_sub_magnitude/*`             | +0 to +2 % (noise) |
| `ibig_hashmap_keys/*`              | new bench, ~19.5 ns/lookup |

Heap-clone regressions (`just_over_inline`, `mid`) are now well within
the user's 5 % threshold and `large` is actively faster than baseline.

## Conclusion of this round

The remaining headroom on hegel-style i128-fitting workloads is in code
the sampling profiler attributes to anonymous frames inside
`map::fold` — i.e., the inlined clone + sub + cmp + dispatch work
itself. Without a deeper profiling tool (e.g., `cargo-pgo` or
hardware-PMC sampling with full debug info), the next-step decision is
hand-rolled: write more targeted micro-benches that pin a specific
ordered chain and read the asm.

For now `shrinker_consider/64` is 1.49× faster than the post-Rec2
baseline and 2.04× faster than the original pre-opt baseline. Code
size is +5.3 % at most on `small_int` (`workload` +1.4 %), well within
the user's accepted budget.

## Open items / future work

- **Tuple drop batching**: `drop_in_place<(IBig×4)>` is 15.7 % of
  shrinker_consider. Each per-IBig drop is at the instruction floor
  (capacity-abs check). The only way to compress further is a
  language-level change (no per-field drop) or a representation
  change. Out of scope.
- **`Buffer::allocate(3)` in the rare add_dword overflow path**: this
  is the heap allocation when two i128-fitting values' sum overflows
  i128. ~25 % of `ibig_sub_magnitude/two_word` operations hit this.
  An inline-direct-alloc shape (same as `clone_heap`) could save a
  few instructions per overflow. Estimated win < 5 % on that
  specific bench. Not pursued; revisit if a real workload shows it.
- **Hash on heap IBig**: hash() walks the slice. For long heap IBigs,
  this is O(words). Not on any of the shrinker hot paths but mentioned
  for completeness.
