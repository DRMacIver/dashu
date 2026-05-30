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

## Where this leaves the next step

The dispatch-chain story from the original profile (Rec 2: inline +
inline fast path, ~7.5 % self / 29 % inclusive in `sum-small`) is
unchanged by this patch. Re-running the profile against the post-opt
binary should now show:

- `Repr::from_buffer` / `Repr::Drop` contributions on `sum-small`
  reduced to noise (the cost they accounted for is gone).
- The inline → heap dispatch and `add::add_dword_in_place` /
  `add::sub_dword_in_place` (the inner kernels themselves) now make
  up a larger fraction of the remaining time — that's where Rec 2
  bites next.

Recommended sequence:

1. Re-profile `sum-small` against the post-opt binary, append the
   new flat profile here.
2. Implement Rec 2 (`AddAssign` short-circuit when both reprs are
   inline) and compare against the same baseline.
3. Then take on Rec 3 (the `_platform_memmove` in `mix-small`),
   informed by an updated inclusive call graph.
