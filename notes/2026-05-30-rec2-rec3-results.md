# Rec 2 (dispatch chain) and partial Rec 3 (memmove)

Continuing from [`2026-05-30-rec1-from-buffer-skip.md`](./2026-05-30-rec1-from-buffer-skip.md).

## Rec 2: collapsing the Add / Sub / SubSigned dispatch chain

### Approach

The post-Rec 1 sum-small profile showed `Add<TypedRepr> for TypedReprRef::add`
at 11.69 % self / 27.66 % inclusive, with all 8 impls in the dispatch
chain already marked `#[inline]`. LLVM was choosing to keep the 4-way
match as an out-of-line call — most likely because the `TypedRepr` enum
is 32 bytes (max of Small=DoubleWord and Large=Buffer, plus the
discriminant), and passing it by value past the inliner's cost
threshold.

Promoting the dispatch impls to `#[inline(always)]` forces fusion. The
8 functions touched (in `integer/src/add_ops.rs`):

- `Add<TypedReprRef> for TypedReprRef`
- `Add<TypedRepr>    for TypedReprRef`
- `Add<TypedReprRef> for TypedRepr`
- `Add<TypedRepr>    for TypedRepr`
- `Sub<TypedReprRef> for TypedReprRef`
- `Sub<TypedReprRef> for TypedRepr`
- `Sub<TypedRepr>    for TypedReprRef`
- `Sub<TypedRepr>    for TypedRepr`
- `SubSigned<...>` four variants

### Quick aside: Drop is `#[inline]` now

The post-Rec1 profile also flagged `Repr::Drop::drop` at 6.8 % self even
though the steady-state drop is on inline-zero (`*self` after `mem::take`
in the `AddAssign` macro expansion). The function body is ~3 instructions
(read capacity, branch on `> 2`, return), but the call boundary blocked
LLVM from folding `mem::take` + assign-back + drop into a single
write-elimination chain. `#[inline]` on both `Drop for Repr` and
`Drop for Buffer` removed that block. Headline effect:
`running_sum_and_compare_small` went from -16.1 % to -32.8 %.

### Results

Cumulative deltas (vs the original pre-opt baseline, sequence
Rec 1 → Drop inline → Rec 2 dispatch `#[inline(always)]`):

| Bench                            | Pre   | +Rec1 | +Drop | +Rec2 |
| -------------------------------- | ----- | ----- | ----- | ----- |
| `running_sum_and_compare_small`  | 0     | -16.1 | -32.8 | **-40.3** |
| `running_sum_and_compare`        | 0     | -9.4  | -14.2 | -22.4 |
| `string_round_trip`              | 0     | ~0    | ~0    | ~0    |
| `bounded_arithmetic_mix`         | 0     | -0.6  | ~0    | ~0    |
| `bounded_arithmetic_mix_small`   | 0     | -2.5  | -2.5  | -2.5  |

The original profile notes predicted 5 – 10 % from Rec 2 on top of
Rec 1; we got 7 – 8 pp on `sum-small`, plus the extra from Drop. The
"by-ref Add"-heavy mix benches stay roughly flat because their hot
path is the memmove discussed below, not dispatch.

### Post-Rec 2 sum-small flat profile

```
self%   incl%  function
67.66  100.00  profile_workload::main      (inlined hot path)
13.21   13.21  add::sub_dword_in_place
12.68   12.68  add::add_dword_in_place
 1.67    1.67  cmp::cmp_in_place           (the bound check)
 0.52    0.52  _platform_memset
 0.38    1.91  Buffer::allocate_exact
 0.33    0.33  Repr::from_buffer
```

Everything except the two inner kernels has been absorbed into `main`.
The kernels (carry-propagation add/sub of a DoubleWord into a Word
buffer) are at the floor of what portable Rust can do here.

## Rec 3: borrowed-Add memmove

### What the profile tells us now

`mix-small` post-Rec 2 still shows:

```
18.62   18.62  add::add_same_len_in_place
16.86   16.86  _platform_memmove
10.50   10.76  mul_ops::mul_large_dword
 8.24    8.24  add::sub_in_place_with_sign
```

The `_platform_memmove` cost is the `Buffer::from(&[Word])` allocation
+ memcpy in the by-ref Add path for `(RefLarge, RefSmall)`. But the
bench multiplies r2 by a small value every 8 iterations
(`r2 = &r2 * black_box(v)`), so r2 grows ~128 bits per multiplication,
reaches ~65 k bits before the next reset (`r2 = &r3 << 1` at i&7==6),
and feeds the by-ref ops with kilobyte-sized buffers. The memmove
work for moving those words is fundamentally O(words) — we cannot
shorten the data movement without changing the API.

### What I tried

`Buffer::from(&[Word])` is `#[inline(always)]` now so LLVM can fuse
the alloc+memcpy with the subsequent `add_dword_in_place` /
`sub_dword_in_place` call site. Saves the call boundary + helps
register allocation across the boundary. Effect on `mix-small` and
`mix`: -1 % each. Marginal but consistent. Committed.

### What I did *not* try, and why

The original profile notes suggested:

> The borrowed/borrowed Add could allocate the result buffer at the
> correct size up front and write into it directly, without the
> intermediate copy.

A fused alloc-and-fill helper for `(RefLarge, RefSmall)` would do
the same total work (alloc + a single read-modify-write pass over the
LHS slice into the result) and avoid one function-call boundary
between `Buffer::from` and `add_dword_in_place`. With both already
`#[inline(always)]`, LLVM should be generating code very close to
that fused shape. I didn't write the explicit fused version because:

- the post-inline-Buffer-from profile shows the memmove cost is still
  ~17 % even though the call boundary is gone — confirming the cost
  is in the actual word-level data movement, not the function-call
  overhead;
- mix-small's memmove pressure comes from an unbounded r2 growth via
  repeated `&r2 * v`, which is a property of the bench's op sequence
  rather than something the library can compress;
- callers can already side-step the cost completely by using `+=` on
  an owned LHS, which Rec 1 has already optimised.

If a future profile against a real hegel-rust trace shows borrowed-Add
on heap LHS as a top hot spot, the fused helper is the next thing to
write. For the synthetic workloads we have, the marginal win didn't
justify the additional code.

## Rec 4: large multiplication

The original notes deferred this as "Note for later, out of scope for
the hegel-rust small-int optimisation goal." Post-Rec 1+2+3 the
`bounded_arithmetic_mix` (full mixed distribution) profile is still
dominated by:

```
~25 %  mul::simple::add_signed_mul_chunk
~22 %  _platform_memmove
~16 %  add::add_same_len_in_place
~10 %  add::sub_in_place_with_sign
 ~7 %  mul_ops::mul_large_dword
 ~5 %  mul::karatsuba::add_signed_mul_same_len
 ~4 %  mul::toom_3::add_signed_mul_same_len
```

These are the multiplication kernel and its supporting carry-add
infrastructure on 100 k-bit operands. Improving the asymptotic cost
of these would mean replacing schoolbook / Karatsuba / Toom-3
with FFT-based multiplication (the `fft` module in `integer/src/mul/`
is partially scaffolded — see the `force_bits` cfg warnings and the
`FFTParams` / `fft_forward` / `fft_reverse` "never used" warnings
that point at unfinished code). That's a multi-week project on its
own, far from the hegel-rust use case which doesn't hit 100 k-bit
multiplications.

**Status: ruled out for this pass.** The note that "improving it
requires deep work in the multiplication module and isn't aligned
with the hegel-rust small-int optimisation goal" still holds; I am
not picking it up in this sequence.

## New opportunities surfaced by the post-Rec 2 profile

### `cmp::cmp_in_place` (1.67 % on sum-small)

The bound check `sum >= bound` runs every iteration. For sums that
are visibly shorter than `bound` (most steady-state iterations), the
length comparison alone settles the ordering. Already early-exits on
length, but the function-call boundary may still be in the way. A
followup `#[inline]` audit on `cmp_in_place` and the `PartialOrd` /
`Ord` impls for `IBig` / `Repr` is the cheapest next thing to look at.
**Not pursued in this round** because 1.67 % is small and the change
might thrash unrelated benchmarks.

### Add/sub kernel SIMD

`add_same_len_in_place` and `sub_in_place_with_sign` are at 18 % +
8 % of `mix-small`. The inner loop is a portable Rust scalar
`overflowing_add` + carry chain that LLVM cannot easily vectorise
(the carry dependency between iterations is the blocker). aarch64
has `adcs`-equivalent intrinsics that would help, but at the cost
of arch-specific code paths. **Noted, not pursued** — outside the
"generically useful" constraint the user set.

### Buffer pooling for hot accumulator workloads

`Buffer::allocate_exact` is at 8.73 % inclusive on `mix-small`. Each
by-ref Add allocates a fresh heap buffer for the result. A real
hegel-rust accumulator that loops `r = &r + v` would benefit from
reusing the dropped buffer for the next allocation. Some bigint
libraries (`malachite`, `rug`) provide a thread-local pool for this.
**Noted as a follow-up** — bigger design change, deserves its own
investigation.

## Cumulative bench summary (vs pre-opt baseline)

After this whole sequence (Rec 1 + Drop inline + Rec 2 + Buffer::from
`#[inline(always)]`):

| Bench                            | Δ        |
| -------------------------------- | -------- |
| `running_sum_and_compare_small`  | **-40.3 %** |
| `running_sum_and_compare`        | -21.4 %  |
| `bounded_arithmetic_mix_small`   | -2.5 %   |
| `bounded_arithmetic_mix`         | -1.0 %   |
| `string_round_trip`              | -0.9 %   |

The hegel-style small-int hot path (sum-small) is now 1.67× faster
than before this work; the mixed-distribution sum (the closest
proxy for a real running-total loop with occasional large operands)
is 1.27× faster.

## Verification

All changes verified with:

- `cargo test -p dashu-int --features rand` — 74 lib tests + integration tests pass.
- `cargo test -p dashu-int --features hegel-tests --test stateful_ibig --test stateful_ubig` — both stateful equivalence machines pass against num-bigint.

The targeted regression tests added during bug discovery
(`test_ibig_shr` cases and `test_be_le_bytes_roundtrip_negative_power_of_two`)
also exercise the modified paths.
