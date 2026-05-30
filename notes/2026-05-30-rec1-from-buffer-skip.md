# Rec 1 landed: skip from_buffer finalisation on heap-stay-heap paths

Implements the first recommendation from
[`2026-05-30-initial-profile.md`](./2026-05-30-initial-profile.md): the
14.78 % allocator-bookkeeping cost on `sum-small` (`Repr::from_buffer`
running `pop_zeros` + `shrink_to_fit` on every Add result, even when
both operations are no-ops) can be bypassed when the operation's shape
guarantees the result is already in canonical heap form.

## Change

`Repr::from_buffer_normalized(buffer)` (new, `pub(crate)`, `#[inline]`):
caller asserts `len >= 3`, top word non-zero, capacity within
`max_compact_capacity(len)`. Skips both `pop_zeros` and `shrink_to_fit`
and transmutes the `Buffer` directly. Debug-asserts the preconditions
so a wrong call site trips in tests.

Call sites rewired in `integer/src/add_ops.rs`:

- `add_dword` (overflow path) — buffer is freshly allocated with a
  pushed top of `1`.
- `add_large_dword` — input is `Large` (`len >= 3`, top non-zero);
  adding a dword into the low words preserves both invariants.
- `add_large` — same reasoning. The same-length operand case is the
  interesting one: both tops are non-zero by invariant so their sum
  can only be zero when there's a carry-out, and then a fresh `1` is
  pushed above.
- `add_large_one` — adding `1` either leaves the top untouched,
  increments a non-`MAX` top to a non-zero value, or cascades to a
  pushed `1`.
- `sub_large_dword`, `sub_large_one` — sub paths can zero the top
  word in one specific case (top was exactly `1` and a borrow
  propagated all the way up). Branch on the post-op top word: take
  the fast path when non-zero, fall back to full `from_buffer`
  otherwise. The fall-back cost is negligible because the branch
  predicts "non-zero" the overwhelming majority of the time on the
  hot loop.

`Repr::from_buffer` itself is now `#[inline]` so the fall-back paths
get specialised at call sites too.

Not touched (magnitude can shrink past the heap boundary):

- `sub_large` (`add::sub_in_place` on multi-word RHS).
- `repr_signed::sub_large` (cancellation can reduce magnitude
  dramatically, e.g. `2^n - (2^n - 1) = 1`).

## Verification

Default `cargo test -p dashu-int` passes. Stateful equivalence tests
against `num-bigint` (`--features hegel-tests`) pass for both `IBig`
and `UBig` — these would have caught any divergence in normalisation
shape across operations.

## Bench deltas (vs `pre-opt` baseline)

Workload scenarios (best-of-three on a quiet machine, criterion
defaults):

| Bench                            | Δ        |
| -------------------------------- | -------- |
| `running_sum_and_compare_small`  | **-16.1 %** |
| `running_sum_and_compare`        | -9.4 %   |
| `bounded_arithmetic_mix_small`   | -2.5 %   |
| `bounded_arithmetic_mix`         | -0.6 %   |
| `string_round_trip`              | ±0 %     |

The headline `sum-small` number is past the upper end of the
predicted 8 – 12 % range. The `mix-small` floor is consistent with
that scenario's dominant cost — `Buffer::from(&[Word])` memmove on
the borrowed/borrowed Add path — being addressed by a separate item
(Rec 3). `string_round_trip` is parsing/formatting bound, so the
Add finalisation change is invisible.

Micro-bench deltas (selected — full set saved in criterion's
`pre-opt` baseline):

The two diagnostic benches that should show the largest movement —
heap accumulator + small RHS — do:

| Bench                                                      | Δ           |
| ---------------------------------------------------------- | ----------- |
| `ubig_add_assign_heap_acc_small_rhs/just_over_inline`      | -17.9 %     |
| `ubig_add_assign_heap_acc_small_rhs/mid`                   | -17.7 %     |
| `ubig_add_assign_heap_acc_small_rhs/large` (~100k bits)    | **-40.7 %** |
| `ibig_add_assign_heap_acc_small_rhs/mid`                   | -8.3 %      |
| `ibig_add_assign_heap_acc_small_rhs/large`                 | -4.4 %      |

The 40 % drop on the 100k-bit `UBig` case is the cleanest signal:
the kernel work (one or two carry-walked words) is unchanged, so the
entire delta comes from the `from_buffer` finalisation we now skip.

Primitive-RHS variants:

| Bench                                       | Δ        |
| ------------------------------------------- | -------- |
| `ubig_add_assign_u64_into_heap_acc`         | -19.7 %  |
| `ubig_add_assign_u128_into_heap_acc`        | -12.8 %  |
| `ibig_add_assign_i64_into_heap_acc`         | -9.9 %   |
| `ibig_add_assign_i128_into_heap_acc`        | -8.9 %   |

By-class same-class binops with a heap result are also faster:

| Bench                                       | Δ        |
| ------------------------------------------- | -------- |
| `ubig_add_assign_by_class/two_word`         | -12.7 %  |
| `ibig_add_by_class/mid`                     | -7.5 %   |
| `ibig_sub_assign_by_class/two_word`         | -9.3 %   |
| `ibig_sub_assign_by_class/zero`             | -10.7 %  |

Inline-only classes (`zero`, `one_word`) measure as noise (±2 %, wide
CIs) since they don't touch the heap path this patch changes.

`ubig_bitxor_assign_by_class/zero` also moves -21.6 %, which is a
side-effect of marking `from_buffer` `#[inline]`: the bitwise impls
in `bits.rs` benefit from cross-callsite specialisation even though
they don't take the explicit `from_buffer_normalized` path.

## Post-Rec1 flat profiles

Same procedure as the original profile, against the post-Rec1 binary.

### `sum-small` (post-Rec1)

```
self%   incl%  function
50.13   99.83  profile_workload::main
16.13   16.13  add::add_dword_in_place
13.76   13.76  add::sub_dword_in_place
 9.75   27.09  Add<TypedRepr> for TypedReprRef::add
 6.80    6.80  Repr::Drop::drop
 0.63    0.63  cmp::cmp_in_place
```

Comparison to pre-Rec1:

- `Repr::from_buffer`: 9.00 % → not in top — vanished into inlined
  callers as designed.
- `main + Add::add` inclusive bucket (combined small-int hot path +
  dispatch): 53.3 % → 59.9 %, a +6.6 pp share growth in a smaller
  total. That's the slack the absorbed `from_buffer` work left.
- `Repr::Drop::drop`: 5.78 % → 6.80 %. Same absolute work; the
  denominator shrank.
- The two inner kernels (`add_dword_in_place`, `sub_dword_in_place`)
  are now the two largest single hot spots after the dispatch chain.

### `mix-small` (post-Rec1)

```
self%   incl%  function
18.14   18.14  add::add_same_len_in_place
16.65   16.65  _platform_memmove
10.93   11.09  mul_ops::mul_large_dword
 8.00    8.00  add::sub_in_place_with_sign
 7.04   99.60  profile_workload::main
 4.12    8.09  shift_ops::shl_large_ref
 1.29    1.29  Repr::Drop::drop
 0.98    0.98  Repr::from_buffer
```

`mix-small` barely shifts. `Repr::from_buffer` dropped from 2.52 % to
0.98 %, but `_platform_memmove` remains at 16.65 % — Rec 3 territory,
and the picture there is unchanged by this patch. That matches the
–2.5 % bench delta.

## Where this leaves the next step

`sum-small`'s remaining ranked hot spots after Rec 1:

1. The dispatch chain (`Add<TypedRepr> for TypedReprRef::add` at
   9.75 % self / 27.09 % inclusive) — what Rec 2 targets directly.
   With the `from_buffer` overhead removed, the relative payoff of a
   shorter dispatch is now larger than the headline 5 – 10 %
   prediction from the original notes.
2. `Repr::Drop::drop` at 6.80 % — `Drop` isn't `#[inline]` on
   either `Repr` or `Buffer`; the call boundary may be visible
   enough to chase with the same low-effort `#[inline]` change.
3. The kernels themselves (`add_dword_in_place`, `sub_dword_in_place`)
   are at the rough floor for portable Rust ALU work and probably
   aren't movable without arch-specific intrinsics.

For `mix-small`, Rec 3 (`_platform_memmove` at 16.65 %) is now
proportionally more of the workload than it was — the right next
step there.

Recommended sequence: Rec 2 on `sum-small`, then a second
re-profile, then Rec 3 informed by the post-Rec2 picture.
