use crate::arch::word::Word;

/// Add a + b + carry.
///
/// Returns (result, overflow).
///
/// Same semantics as the generic version, but written so LLVM can fuse
/// chained calls into a single `adds` / `adcs` carry chain on aarch64.
///
/// The generic implementation does two separate `overflowing_add` calls
/// with the input carry threaded through an integer register; LLVM then
/// emits `adds + cset + adcs + cset` (5 instructions for a 2-word chain,
/// + an explicit OR of the two captured carries). With this version,
/// the inline asm sets the C flag from the input carry and uses a single
/// `adcs`, so a chained call site emits the minimal `adds + adcs + cset`
/// per word pair.
#[inline]
pub fn add_with_carry(a: Word, b: Word, carry: bool) -> (Word, bool) {
    let result: Word;
    let new_carry: Word;
    // SAFETY: this asm has no memory side effects and reads / writes only
    // the indicated registers + the NZCV condition flags (which we clobber).
    unsafe {
        core::arch::asm!(
            // Set the C flag from `carry`: subs of `carry - 1` produces
            // C = !borrow = (carry == 1). xzr is the zero register; the
            // result of the subtraction is discarded into it.
            "subs xzr, {carry}, #1",
            // a + b + C.
            "adcs {result}, {a}, {b}",
            // Capture the post-add C flag as a 0/1 word.
            "cset {new_carry}, hs",
            a = in(reg) a,
            b = in(reg) b,
            carry = in(reg) carry as Word,
            result = lateout(reg) result,
            new_carry = lateout(reg) new_carry,
            options(pure, nomem, nostack),
        );
    }
    (result, new_carry != 0)
}

/// Subtract a - b - borrow.
///
/// Returns (result, overflow).
///
/// Mirrors `add_with_carry` for the subtract side. aarch64 uses an
/// inverted C-flag convention for sub (C = !borrow), hence the
/// `subs ... !borrow ... sbcs ... cset lo` shape below.
#[inline]
pub fn sub_with_borrow(a: Word, b: Word, borrow: bool) -> (Word, bool) {
    let result: Word;
    let new_borrow: Word;
    // SAFETY: as above.
    unsafe {
        core::arch::asm!(
            // Set the C flag to !borrow (since aarch64 sub uses
            // C = !borrow): subs of (!borrow) - 1 reads C as the inverted
            // input borrow.
            "subs xzr, {nborrow}, #1",
            // a - b - !C; on aarch64 sbc subtracts (rhs + !C).
            "sbcs {result}, {a}, {b}",
            // Capture the post-sub borrow (C == 0).
            "cset {new_borrow}, lo",
            a = in(reg) a,
            b = in(reg) b,
            nborrow = in(reg) (!borrow) as Word,
            result = lateout(reg) result,
            new_borrow = lateout(reg) new_borrow,
            options(pure, nomem, nostack),
        );
    }
    (result, new_borrow != 0)
}
