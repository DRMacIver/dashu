# len == 3 fast-path experiment — negative result

User suggestion: the various heap-Add helpers all require `buffer.len() >= 3`.
The hegel-style running sum workloads spend most of their time in heap
buffers near that boundary, so a `len == 3` specialisation could plausibly
win.

## What the workload actually does

Empirical measurement (`integer/examples/measure_lens.rs`, since deleted)
across one full run of the `sum-small` profile scenario:

| `sum.as_sign_words().1.len()` | Count   | %      |
| ----------------------------- | ------- | ------ |
| 0                             | 32      | 0.02   |
| 2                             | 20,128  | 15.36  |
| 3                             | 110,912 | **84.62** |

So **84.6 %** of `sum += v` iterations hit `add_large_dword` with
`buffer.len() == 3`. (The 0/2 cases are the early iterations before the
running sum crosses the inline boundary; once heap-resident, the
accumulator stays at exactly 3 words for the remainder of the loop
because the random walk's typical magnitude is `~i128 * sqrt(N)` ≈ 134
bits, which is 3 64-bit words.)

That's a strong signal the specialisation could matter.

## What was tried

Two variants of an inline 3-word carry chain in `add_large_dword`:

1. **Two-exit shape**: separate `return` from the specialised arm,
   leaving the general-path arm untouched.
2. **Single-exit shape**: compute `overflow: bool` in both arms and
   share the post-add `push_resizing` + `from_buffer_normalized`
   tail.

Specialisation body (variant 2):

```rust
let overflow = if buffer.len() == 3 {
    let (b0, b1) = split_dword(rhs);
    let (s0, c0) = buffer[0].overflowing_add(b0);
    let (s1, c1) = crate::arch::add::add_with_carry(buffer[1], b1, c0);
    let (s2, c2) = buffer[2].overflowing_add(c1 as Word);
    buffer[0] = s0;
    buffer[1] = s1;
    buffer[2] = s2;
    c2
} else {
    add::add_dword_in_place(&mut buffer, rhs)
};
```

## Result: net regression on `sum-small` of ~2 pp

| Bench                            | Before specialisation | With (variant 1) | With (variant 2) |
| -------------------------------- | --------------------- | ---------------- | ---------------- |
| `running_sum_and_compare_small`  | -50 %                 | -48.2 %          | -48.5 %          |
| `running_sum_and_compare`        | -22 %                 | -20.1 %          | -20.5 %          |

The profile after specialisation:

```
85.30  100.00  main
 9.43    9.43  add_large_dword     (was 12.72)
 0.55    0.55  _platform_memset
 0.46    0.46  Repr::from_buffer
```

`add_large_dword`'s self time DID drop (12.72 → 9.43 %) — but more time
moved into `main` (82.41 → 85.30 %), so the total grew. The specialisation
shaved cycles from the kernel but added enough branch overhead + code
duplication on the path through `main` to lose net.

## Why it didn't help

Reading the post-Rec 2 disassembly of `add_large_dword`, LLVM was already
generating an `adcs` chain through the kernel:

```
ldp   x10, x11, [x9]      ; load buffer[0..2]
adds  x11, x11, x3        ; word_1 + rhs_hi (sets C)
cset  w12, hs             ; save C
adds  x13, x10, x2        ; word_0 + rhs_lo (sets C)
adcs  x11, x11, xzr       ; word_1 += carry from word_0
cset  w10, hs             ; save C
stp   x13, x11, [x9]      ; store back
```

That's the same instruction-level sequence the hand-rolled
specialisation would emit. The remaining work is the carry-propagation
loop in `add_one_in_place(words_hi)`, which for `len == 3` is 1
iteration of `ldr / adds / str` — also already tight.

The branch on `buffer.len() == 3` itself, plus the duplicated
post-add tail, was net slightly negative — LLVM can't merge the two
paths cleanly without losing optimisation opportunities on each.

## What *did* help on the kernel

Removing the redundant `Option::None` arms of the two `split_first_mut`
calls inside `add_dword_in_place` (and `sub_dword_in_place`) via a
`core::hint::unreachable_unchecked` hint. Visible asm diff:

```
-cbz   x8, LBB32_14
-cmp   x8, #1
-b.eq  LBB32_15
 ldr   x9, [x19]
 ldp   x10, x11, [x9]
```

Three predicted-not-taken branches removed from the entry path. The
runtime impact was within bench noise (mispredicts on those branches
were already rare), but the code-size reduction is concrete.

## Conclusion

The hypothesis that a specialised len-3 path could win was reasonable,
but the current generic code is already at LLVM's ceiling for this
shape (modulo the minor bounds-check elision above). The empirical
84 % concentration on `len == 3` doesn't translate into headroom
because the per-iteration cost difference between the specialised and
generic paths is sub-cycle.

If someone in the future writes an arch-specific `add_dword_with_carry`
that does the full 3-word chain as 4 explicit `adcs` instructions and
shaves the leading `cset/adcs/cset` pair the current generic code
emits, that would have ~1-2 % to win. The portable-fallback shape is
clear; the win is too small to justify the duplication for now.
