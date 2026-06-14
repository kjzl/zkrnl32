//! Bit-mask construction helpers for the `u32` bitmap words.
//!
//! The helpers in this file are intentionally policy-free: callers decide
//! whether a bit means free, used, present, writable, or anything else. They
//! are specialised to `u32` because every bitmap in the kernel uses `u32`
//! words; if another word width is ever needed, introduce a small local word
//! trait here rather than pulling in an external crate.

pub const WORD_BITS: usize = u32::BITS as usize;

/// Builds a contiguous bit mask clipped to one `u32` word.
///
/// `start_bit` is the first bit index to include in the word, where bit `0` is
/// the least-significant bit. `len` is the requested number of bits. If the
/// requested range would cross the end of the word, the mask is clipped to the
/// bits that still fit.
///
/// Returns the mask and the number of bits represented by that mask.
///
/// # Panics
///
/// In debug builds, panics if `start_bit >= 32`.
///
/// # Examples
///
/// ```
/// # use zkrnl32::utils::bits::clipped_bit_range_mask;
/// let (mask, taken) = clipped_bit_range_mask(29, 8);
///
/// assert_eq!(taken, 3);
/// assert_eq!(mask, 0b111 << 29);
/// ```
#[inline(always)]
pub fn clipped_bit_range_mask(start_bit: usize, len: usize) -> (u32, usize) {
    debug_assert!(start_bit < WORD_BITS);

    let take = len.min(WORD_BITS - start_bit);

    if take == 0 {
        return (0, 0);
    }

    let end_bit = start_bit + take;
    let mask = (u32::MAX << start_bit) & (u32::MAX >> (WORD_BITS - end_bit));

    (mask, take)
}
